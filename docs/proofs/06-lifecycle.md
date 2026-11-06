# Ticket 06: native exit, stop, quit and detach fixture proof

2026-11-06; macOS 26.2 (25C56), arm64, stable rustc 1.99.1, debug build,
portable-pty 0.9, Crossterm 0.29 and vt100 0.18. Real 160×40 outer PTYs drive
actual dashboard/supervisor/scripted-child processes in temporary real Git
checkouts and isolated user-state/socket directories under `/private/tmp`.
The outer terminal is the automated PTY/VT parser, not a terminal application.
Neither Claude Code nor Codex CLI was launched. No durable history is claimed.

## Behavior and boundaries

- Native `exit` returns focus to the dashboard, reports process `Exited`, retains
  the latest captured screen, adopted checkout and task/session attribution.
  A new explicit launch creates a new session for the same adopted task.
- `s` from dashboard focus keeps the dashboard open, reports `Stopping`, sends
  TERM to the managed process group and escalates to KILL after 500 ms. It
  confirms the leader's exit AND absence of executing group members before
  showing `Stopped`. The fixture launches a descendant in the same group; both
  leader and descendant ignore TERM. External `ps` evidence verifies that the
  descendant cannot execute after stop. Zombies are not executing processes.
  Working-change file contents survive native exit and explicit stop unchanged.
- The termination worker has a two-second overall polling deadline, with an
  additional at-most-500-ms verification subprocess timeout. Inspection errors,
  timeout or observable survivors become `StopFailed`, preserve the reservation
  and show retry guidance. A fixture PATH-scoped failing `ps` executable proves
  that ordinary quit stays visible instead of exiting on uncertain evidence.
  Repairing that executable and pressing `q` confirms shutdown and removes only
  the isolated socket. No arbitrary PID-based survivor cleanup is performed.
- Ordinary `q`/Esc/Ctrl-C in dashboard focus enters pending shutdown. App exits
  only after a confirmed supervisor shutdown observation. That final observation
  is reliably delivered rather than discarded as a replaceable screen snapshot.
  A state test covers pending, failure, explicit retry and confirmed completion.
- `d` exits through the normal terminal guard, explicitly leaves alternate screen
  and disables outer bracketed paste, and preserves the running supervisor and
  child. Reopening answers another prompt in the same child. Unexpected dashboard
  kill likewise preserves execution and permits a replacement controller, as the
  ownership scenario verifies. Ctrl-]/Ctrl-5 changes focus only.
- A separate bounded input writer has 32 slots of at most 4096 bytes; the
  supervisor never blocks on its PTY write. A fixture stops reading stdin;
  100 requests of 4096 bytes actually saturate the writer queue and surface
  retryable backpressure. Shutdown still confirms within the test's three-second
  bound. This is rejected-input backpressure, not a transcript-storage guarantee.
- The UI labels process lifecycle separately from semantic agent status `unknown`.
  Failure attribution, stop, detach and quit actions remain visible. No worktree
  cleanup or captured-history deletion occurs. This slice retains an ephemeral
  screen; ticket 08 still owns durable metadata/history and shutdown flush.

Cleanup covers the managed process group, including its ordinary descendants.
Processes deliberately escaping it with `setsid`/`setpgid` are outside this proof;
this is not arbitrary descendant containment or supervisor-crash reconciliation.
Native-agent tickets must establish their actual process ownership. Ticket 09
still owns uncertain survivors and launch-result reconciliation.

## TDD and validation

The approved seams are the real dashboard/supervisor/scripted PTY plus existing
App/render boundaries. The pending-shutdown test initially failed to compile
because that state did not exist. The native-exit scenario initially failed for
missing visible `Process: Exited`; the blocked-reader scenario initially failed
for missing `input paused`. Implementation made each behavior pass. The failure
scenario exposed missing visible task attribution during errors; attribution now
remains visible. Reliable final stop delivery also closes a snapshot-loss race.

An initial sandbox run could not bind its isolated supervisor socket; all actual
PTY/socket proof runs used approved execution outside the sandbox. Earlier full
runs exposed host-sensitive initial discovery timeouts between unrelated outer
PTY scenarios; the harness now serializes independent scenarios while preserving
intentional concurrency inside ownership races. A verification-script timeout
also correctly produced `StopFailed`; failure expectations accept either failed
inspection or its bounded timeout, and repair uses `/bin/ps` directly.
These observations are not native startup-latency measurements.

Final reproduction commands, all successful:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets -- --nocapture
```

Raw result summary from the final run:

```text
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.18s
13 passed; 0 failed; 1 ignored (subprocess probe driven by restoration test)
TERM-refusing descendant stop confirmed in 633.268834ms
Saturated-input shutdown confirmed in 87.496458ms
8 passed; 0 failed; 0 ignored; finished in 11.19s
1 passed; 0 failed; 0 ignored; finished in 2.15s
```

Timing samples are single observations from `Instant` around the action and
visible confirmation (descendant) or protocol confirmation (saturated input).
They demonstrate the specific termination bounds with assertion/PTY overhead;
they are not distributions, output/resource measurements or milestone-wide
latency acceptance. Native compatibility, arbitrary terminals, durable flush,
crash recovery and storage-fault responsiveness remain unverified here.
