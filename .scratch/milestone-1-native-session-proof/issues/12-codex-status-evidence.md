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

## Comments

- 2026-11-06: Implemented and recorded all five native Codex 0.167.0 status
  signals in [the versioned proof](../../../docs/proofs/12-codex-status.md), with
  matching native PTY screens and hook receipts. Native Stop continuation,
  quoted status phrases, stale question evidence and delayed older-turn evidence
  were exercised; native exit and explicit supervisor shutdown completed.
  Shared fixture/App/render checks cover labels, unavailable/malformed evidence
  and continued activity after an intermediate failed command.
- Branch `ticket/12-codex-status`, implementation/docs through `5c7fc9d`, includes
  execution gate 7455825, Claude status daf1005 and transport checkpoint cc97e30.
  Locked build, formatting and warnings-denied Clippy passed. A full suite before
  the transport fix exposed the existing bounded-history partial-frame EOF;
  after cc97e30 that test and the Codex regression passed. The final full suite
  then stalled in the old bounded-PTY test cleanup. It was explicitly interrupted
  (Cargo PID 32933, test runner 33145); only its known fixture supervisor 33670
  and child 33672 at `/private/tmp/.tmpE15Y5o/state` were stopped, with
  stopped=true and all four PIDs absent. No other agent's test run was stopped.
- Ticket remains **unresolved pending integrated full checks**. Ticket 13's newer
  cleanup checkpoint a3841db is ready for the integrating agent to merge and
  verify; that agent owns the final checks and completion decision. No native
  status signal remains an unresolved proof gap in the declared scenarios.
