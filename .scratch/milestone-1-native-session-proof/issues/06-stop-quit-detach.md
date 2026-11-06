# 06: Stop, quit, and detach predictably

Status: ready-for-agent
Resolved: yes
Blocked by: 02, 03

**What to build:** Choose whether to stop execution or preserve it when leaving the dashboard, while retaining task context and working files.

- [x] Native agent exit ends that session without deleting task context, history already captured, or checkout contents. Returning from attached focus only changes input ownership.
- [x] Explicit stop terminates the managed session with bounded escalation and process-group cleanup. Exercise a descendant process and refusal/failure cases; do not report a confirmed stop until execution state supports it.
- [x] Ordinary quit with the milestone’s default behavior requests supervisor shutdown and stops managed activity. Explicit dashboard detach leaves the child running and restores the dashboard terminal.
- [x] Unexpected dashboard/client loss does not invoke ordinary quit or stop the child. Existing ownership rules allow control to be acquired after client death; full screen reconnection is delivered by its dedicated ticket.
- [x] The fixture demonstrates native exit, stop, quit, detach, client kill, descendant cleanup and visible stop failures. Working file contents and adopted task/session attribution survive each applicable operation.
- [x] Action labels and help explain each operation, using process lifecycle separately from unknown semantic agent status. Durable history flushing is added by the history ticket rather than claimed here.


## Completion

Implemented and fixture-verified on macOS: native exit, explicit stop, ordinary
quit with pending/confirmed shutdown, detach with terminal restoration, client
kill/reacquisition, TERM-refusing managed-group descendant cleanup, visible
verification failure and retry, and responsive stop under saturated PTY input.
Task attribution and working changes survive applicable operations. Lifecycle
and unknown semantic status are separate. Stable locked build, format, Clippy
and all-target tests pass; see [proof and raw result summary](../../../docs/proofs/06-lifecycle.md).
Cleanup is bounded to the managed process group, not intentionally escaped
processes. Durable history/flush remains ticket 08; no native CLI support claim.
