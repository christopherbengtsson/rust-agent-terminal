# 10: Show evidence-based Claude Code status

Status: ready-for-agent
Blocked by: 05

**What to build:** See dependable Claude Code attention and lifecycle indicators, with unknown state when native PTY-compatible evidence cannot establish the answer.

- [ ] For the tested Claude Code version, demonstrate evidence for working, input request, approval request, completed turn and interruption while preserving native PTY interaction. Record the actual signal source, validity conditions and limitations.
- [ ] Display semantic status separately from process liveness and connection state; distinguish stopped/failed execution from turn completion and task archival.
- [ ] Missing, invalid or no-longer-reliable evidence produces unknown. Silence, a live process or a failed intermediate command cannot independently imply working, waiting, completed or failed session state.
- [ ] Drive visible indicators with fixture evidence and deterministic state/event tests, including missing or invalid signals and a failing test while the agent continues working.
- [ ] Record real macOS scenarios for every required signal and compare visible attention/unknown states with the image’s readable-label direction. Signals available only in another integration mode do not establish native PTY compatibility.
- [ ] If dependable required signals cannot be demonstrated, retain an explicit support-gate failure and proposed resolution rather than substituting unsupported inference. Update tested-version documentation.
