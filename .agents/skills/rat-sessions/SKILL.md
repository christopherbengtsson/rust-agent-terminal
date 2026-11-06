---
name: rat-sessions
description: Change or validate managed agent launches, PTY input/output, supervisor ownership, process cleanup, native status, reconnect, or resume in this project.
---

# Managed native sessions

Paths are repository-relative.

1. Locate the changed boundary in `src/session.rs` and `src/events.rs`, then
   select its owner and evidence below. For a request/acknowledgement change,
   trace identity and completion through App, transport, supervisor and worker;
   account for a lost reply. An encoder-only change needs its input path.
2. Exercise the boundary using the existing dashboard/supervisor/scripted-child
   seam in `tests/ownership.rs`, `tests/scripted_session.rs`, or
   `tests/session_screen.rs`. Use an isolated temporary checkout and explicit
   private `--state-dir`; include affected failure windows, not unrelated ones.
3. For native compatibility claims, record actual macOS/tool versions,
   source/binary, argv, workload, observations and owned-process cleanup.
   Report fixture results and unverified native behavior separately.

## Boundaries

- **Execution ownership/startup:** when changing the supervisor interface,
  bootstrap, credentials or OS process handling, read the
  [execution-machine boundary](../../../docs/architecture.md#execution-machine-boundary-direction-local-execution-today).
- **Launch/reconnect/recovery:** inspect `src/recovery.rs`; read
  [ADR 0001](../../../docs/adr/0001-user-wide-supervisor.md),
  [ADR 0005](../../../docs/adr/0005-reconcile-uncertain-session-launches.md) and
  [launch-crash evidence](../../../docs/proofs/09-launch-reconciliation.md).
  Use the existing request-outcome/reconciliation path before considering retries.
- **Input:** inspect `src/native_input.rs`; read
  [input evidence](../../../docs/proofs/03-native-input.md).
  Use `$rat-tui` when changing dashboard focus policy.
- **Screen/resize:** read
  [screen evidence](../../../docs/proofs/04-session-screen.md);
  use `$rat-tui` when changing viewport policy.
- **Semantic status:** inspect `src/native_status.rs` or `src/codex_status.rs`;
  read [ADR 0003](../../../docs/adr/0003-evidence-based-agent-status.md) and the
  corresponding [Claude](../../../docs/proofs/10-claude-status.md) or
  [Codex](../../../docs/proofs/12-codex-status.md) proof, including negative cases.
- **Native resume:** read [recovery evidence](../../../docs/proofs/16-native-recovery.md)
  for conversation identity and readable-history independence.
- **Stop/quit:** inspect `src/lifecycle.rs` and `src/child_scope.rs`; read
  [native-child failure/correction](../../../docs/proofs/18-native-child-stop.md)
  and [quit retests](../../../docs/proofs/18-post-review-native-quit.md).
  Establish cleanup from live ownership evidence; restored PIDs confer no signal
  authority. Use `$rat-persistence` when EOF/final-capture completion changes.
- **Queues/parser/resources:** trace each changed buffer's payload cap, blocked
  owner, saturation and control progress. Read
  [subscriber/parser evidence](../../../docs/proofs/13-slow-subscribers.md) and
  [measurement methodology](../../../docs/proofs/17-session-measurements.md).
  Check Cargo features as well as queue constants when reassessing memory bounds.

Use [integrated evidence](../../../docs/proofs/18-milestone-evidence.md) to scope
qualification claims to the recorded versions, sources and workloads.
