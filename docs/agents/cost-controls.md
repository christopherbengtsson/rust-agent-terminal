# Agent cost controls

These rules apply to agents working on this repository and every real-provider
development, test, milestone proof, PTY, and skill-router check they launch.

- Prefer fixtures, mocks, local skill discovery, and authentication metadata.
  Use real inference only when it is necessary to verify provider behavior.
- Explicitly select the cheapest compatible model and lowest supported reasoning
  on every live invocation. Never rely on account, user-config, or CLI defaults.
  [The test profile](../../scripts/agent-test-profile.json) owns the current
  model, reasoning, and smoke-budget settings. The native launcher embeds it
  only in qualification builds; the smoke runner reads the same file. Change its version ID
  when changing settings, rebuild, and restart qualification supervisors.
  These are compatibility pins, not a guarantee that they remain the cheapest
  models forever. Check pricing and availability before
  selecting a replacement; use a cheaper compatible model when available.
- If a baseline model or reasoning setting is unavailable, stop that live check
  and report it. Do not fall back automatically to a larger model or higher
  reasoning. Get explicit user approval before any more expensive configuration,
  including a test that specifically requires it.
- Keep smoke prompts short, disable tools when possible, limit turns, and stop
  after one successful request per required scenario. Do not run billable retry
  loops, parallel provider tests, or broad live suites without an agreed budget.
- Claude smoke checks limit turns and session spend; a session budget is not
  a guarantee of an exact provider billing ceiling.
  Codex CLI has no equivalent dollar cap here; keep requests minimal and use
  provider-side spend controls where available. Reasoning limits alone do not
  cap total spend.
- Record model IDs, reasoning settings, request counts, and remaining unverified
  behavior when reporting live checks. Never print credentials.

Use `sh scripts/cloud-agent-check.sh` for non-billable local status and add
`--live` only when a real request is needed. Interactive app tests must select
the same cheap model/settings in the agent CLI before submitting a prompt.
These test rules do not change the application's users' chosen agent models.

## Native qualification

Build a qualification binary explicitly:

```sh
cargo build --locked --features native-agent-qualification
```

This feature is off by default. Ordinary builds, including production release
builds, ignore `RAT_AGENT_TEST_PROFILE` and omit test-profile evidence from their
public observations. They do not embed or read the test settings JSON. Keep the
feature disabled for production artifacts.

The milestone workflow driver enables the test profile for new supervisors.
Before every `create` or `controls` keyboard plan, it verifies the supervisor's
live launch profile and the profile of every running session. An older owner
or a running session with unknown settings blocks the plan before input.
Use the compatible dashboard to stop old activity safely before starting a fresh
qualification run; the driver does not replace existing owners or kill sessions.

In test mode, the native launcher passes explicit model/reasoning options before
any first prompt, on both new launches and native resume. Session profile evidence
comes from this process's launch configuration, never saved metadata. It proves
the requested settings, not provider acceptance or actual billing. A CLI error
still fails the live test. Smoke turn/budget flags apply only to smoke requests;
interactive qualification still needs a bounded run and an agreed budget.

For manual app tests, start a fresh disposable supervisor with
`RAT_AGENT_TEST_PROFILE` set to the profile's `id` before any native launch.
Check the public observation's `native_test_launch_profile` and each running
session's `native_test_session_profile` against that ID before sending prompts.
Ordinary app sessions and account configuration are unchanged when the variable
is absent. Existing running sessions keep their original settings and must pass
the per-session check; changing the dashboard's environment cannot change them.
