"""Non-billable checks for native launch preflight and smoke settings."""
import importlib.util
import io
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import unittest


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(filename))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


workflow = load("native_workflow", "m2-19-native-workflow.py")
settings = load("agent_test_profile", "agent_test_profile.py")
PROFILE = settings.profile()["id"]


class NativeWorkflowCostGuards(unittest.TestCase):
    def test_old_or_unconfigured_supervisor_blocks_billable_phases(self):
        for phase in ("create", "controls"):
            for state in ({}, {"native_test_launch_profile": "other"}):
                with self.subTest(phase=phase, state=state):
                    with self.assertRaisesRegex(RuntimeError, "blocked before input"):
                        workflow.require_cost_safe_phase(phase, state, PROFILE)

    def test_existing_running_session_requires_its_own_launch_evidence(self):
        for profile in (None, "other"):
            state = {"native_test_launch_profile": PROFILE, "sessions": [
                {"lifecycle": "Running", "native_test_session_profile": PROFILE},
                {"lifecycle": "Running", "native_test_session_profile": profile},
            ]}
            with self.assertRaisesRegex(RuntimeError, "existing running session"):
                workflow.require_cost_safe_phase("controls", state, PROFILE)

    def test_verified_running_and_interrupted_sessions_can_proceed(self):
        state = {"native_test_launch_profile": PROFILE, "sessions": [
            {"lifecycle": "Running", "native_test_session_profile": PROFILE},
            {"lifecycle": "Interrupted"},
        ]}
        for phase in ("create", "controls"):
            workflow.require_cost_safe_phase(phase, state, PROFILE)

    def test_non_prompt_phases_remain_available(self):
        for phase in ("register", "editors", "archive"):
            workflow.require_cost_safe_phase(phase, {}, PROFILE)

    def test_smoke_requests_use_shared_settings_and_report_either_failure(self):
        for failed_agent in ("codex", "claude", None):
            calls = []
            def run(argv, **kwargs):
                calls.append((argv, kwargs))
                return SimpleNamespace(returncode=int(argv[0] == failed_agent))
            with patch.object(settings.shutil, "which", return_value="fixture"), patch.object(settings.subprocess, "run", side_effect=run), patch("sys.stdout", new_callable=io.StringIO):
                self.assertEqual(settings.smoke(), int(failed_agent is not None))
            self.assertEqual(len(calls), 2)
            profile = settings.profile()
            for argv, kwargs in calls:
                self.assertEqual(argv[1:-1], settings.smoke_command(argv[0], profile)[1:])
                if argv[0] == "claude":
                    for name, value in profile["claude_env"].items():
                        self.assertEqual(kwargs["env"][name], value)
