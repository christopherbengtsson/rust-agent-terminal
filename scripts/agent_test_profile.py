"""Shared settings for provider smoke tests and native qualification launches."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def profile():
    return json.loads(Path(__file__).with_name("agent-test-profile.json").read_text())


def smoke_command(agent, settings):
    if agent == "codex":
        return [agent, "--no-daemon", "exec", *settings["codex_args"],
                "--ephemeral", "--skip-git-repo-check", "--sandbox", "read-only"]
    return [agent, "--print", *settings["claude_args"], *settings["claude_smoke_args"]]


def smoke():
    settings = profile()
    prompt = "Authentication smoke test. Do not use tools or read or write files. Reply only: OK"
    failed = False
    with tempfile.TemporaryDirectory() as directory:
        for agent in ("codex", "claude"):
            if not shutil.which(agent):
                print("Missing CLI: " + agent, file=sys.stderr)
                failed = True
                continue
            command = smoke_command(agent, settings)
            print("Live provider check: " + repr(command), flush=True)
            env = dict(os.environ)
            if agent == "claude":
                env.update(settings["claude_env"])
            result = subprocess.run([*command, prompt], cwd=directory, env=env)
            failed |= result.returncode != 0
    return int(failed)


if __name__ == "__main__":
    if sys.argv[1:] != ["smoke"]:
        sys.exit("Usage: agent_test_profile.py smoke")
    sys.exit(smoke())
