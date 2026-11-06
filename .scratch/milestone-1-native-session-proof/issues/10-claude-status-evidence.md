# 10: Show evidence-based Claude Code status

Status: ready-for-agent
Blocked by: 05
Resolved: yes

**What to build:** See dependable Claude Code attention and lifecycle indicators, with unknown state when native PTY-compatible evidence cannot establish the answer.

- [x] For the tested Claude Code version, demonstrate evidence for working, input request, approval request, completed turn and interruption while preserving native PTY interaction. Record the actual signal source, validity conditions and limitations.
- [x] Display semantic status separately from process liveness and connection state; distinguish stopped/failed execution from turn completion and task archival.
- [x] Missing, invalid or no-longer-reliable evidence produces unknown. Silence, a live process or a failed intermediate command cannot independently imply working, waiting, completed or failed session state.
- [x] Drive visible indicators with fixture evidence and deterministic state/event tests, including missing or invalid signals and a failing test while the agent continues working.
- [x] Record real macOS scenarios for every required signal and compare visible attention/unknown states with the image’s readable-label direction. Signals available only in another integration mode do not establish native PTY compatibility.
- [x] If dependable required signals cannot be demonstrated, retain an explicit support-gate failure and proposed resolution rather than substituting unsupported inference. Update tested-version documentation.

## Comments

2026-11-06 — Partial implementation and native 2.1.318 evidence recorded in
[the status proof](../../../docs/proofs/10-claude-status.md). Correlated native
question and Bash approval receipts/current modals produce Input/Approval.
Working, completed-turn and interruption classifier/negative-proof gates remain
open; neither Stop, a live process, failing command, quiet PTY nor forwarded
Ctrl-C substitutes. Native interrupted acknowledgement was reproduced, but a
current-generation/quoted-marker classifier has not been validated. No claim
that the remaining gates are infeasible is made. Fixture/state/PTY checks pass.
The ticket intentionally remains unresolved pending the documented concrete
signal and negative-case work.

2026-11-06 — Completed bounded Working, Completed and Interrupted classifiers
and the required positive/negative native proof. Final canonical 2.1.318 runs
demonstrated all five signals with the final submitted-turn/timestamp guard;
first Stop under one-shot continuation stayed Unknown, modal cancellation and
quoted old markers did not invent interruption/completion, and a real failed
Bash command retained Running process lifecycle. Delayed prior-turn and stale
receipts, old markers/acknowledgments, active-tool interrupt, fixtures and state/
render behavior are covered at the approved seams. Latest integration `e2cfef3`
is merged; build/fmt/clippy, full pre-parser-merge suite and final merged focused
checks pass as detailed in [the final proof](../../../docs/proofs/10-claude-status.md).
Native status support is established only for the recorded version/scenarios;
unknown/layout/expiry limits remain explicit. Every isolated supervisor was
explicitly shut down. Ticket 10 is resolved; no whole-release support claim is
implied.
