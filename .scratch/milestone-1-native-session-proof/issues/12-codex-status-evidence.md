# 12: Show evidence-based Codex CLI status

Status: ready-for-agent
Blocked by: 11

**What to build:** Identify Codex work needing attention using demonstrated native signals, preserving unknown whenever evidence is unavailable.

- [ ] For the tested Codex version, demonstrate PTY-compatible evidence for working, input, approval, turn completion and interruption; record sources, validity conditions and limitations for each.
- [ ] The dashboard distinguishes semantic state, process liveness, connection status and stopped/failed execution. Completion does not archive the task.
- [ ] Absent, invalid or unreliable signals produce unknown. Silence and failed intermediate commands do not invent attention, completion or session failure.
- [ ] Use the shared fixture/state-event contract to verify visible transitions, missing-evidence handling and continued activity after a failed child command, without an adapter-specific mock framework.
- [ ] Record separate real macOS evidence for every required Codex signal. Evidence available only outside native PTY mode cannot pass the gate; unresolved signals remain explicit proof failures.
- [ ] Use readable text labels alongside color and document exact supported versions and limitations.
