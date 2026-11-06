# Ticket 07: dashboard recovery and launch acknowledgement proof

2026-11-06; macOS 26.2 (25C56), arm64, stable rustc 1.99.1, debug build,
portable-pty 0.9, Crossterm 0.29 and vt100 0.18. Automated 160×40 outer PTYs
exercise real dashboards, supervisors and scripted children in temporary real
Git repositories and private isolated state directories under `/private/tmp`.
This is fixture evidence; neither native coding CLI compatibility nor supervisor
crash reconciliation is established.

The supervisor supplies the initial request UUID before launch and retains launch
outcomes for its lifetime. Every greeting includes the current request identity
and latest result; App restores that identity on reopening. A same-ID retry or
`QueryLaunch` recovers the recorded result, including a failed launch. Explicit
launch after an acknowledged outcome creates a new intent. The bounded result
map refuses further launches at 1024 records instead of evicting an outcome;
request IDs are limited to 128 bytes. No automatic uncertain launch replay occurs.
Ticket 08 adds durable records; ticket 09 owns cross-supervisor reconciliation.

The ownership harness now covers:

- Send launch and close the controller without reading its acknowledgement.
  Reconnect, query the original UUID, retry it and observe the original successful
  result and session identity. The process table contains exactly one supervisor
  for the isolated endpoint and one fixture child belonging to it. Reopening the
  real dashboard continues input in that child.
- Lose the acknowledgement of launch before adoption. After reconnect, recover
  the original failure, adopt a usable checkout and retry the same UUID. It
  returns that exact failure with no session identity and process lifecycle Idle.
- Start 200 numbered output lines at 5 ms intervals, kill the real dashboard
  while output is visible, and acquire the released connection. Consecutive
  coherent styled snapshots retain the same identity and monotonically increasing
  output-byte boundary; output advances after takeover and reaches line 0199
  once. Resize the PTY/parser to 64×18 and verify the same child reports that
  geometry. Reopen the dashboard, request its identity and verify the original
  child PID, dashboard focus, Git refresh/navigation availability and clean quit.
- The existing concurrent-dashboard scenario still refuses a live competing
  controller. The original dashboard continues receiving child responses and
  the process table still contains one owner/child. Kill/reopen takes control of
  that same child. Explicit detach is covered separately by lifecycle tests.

Only periodic `Screen` observations coalesce. Every control-command response,
including launch, failure and final shutdown, uses reliable delivery through the
bounded UI event channel. This prevents a full snapshot slot from discarding a
control result; it does not promise delivery to a dead client. Reopening recovers
owner-lifetime results independently of that acknowledgement.

The lost-ack test first failed at the real external seam because the greeting had
no stable request identity. Implementing request/result retention made it pass.
An initial sandbox execution could not bind the private Unix socket; all actual
socket/PTY checks ran with approved execution outside the sandbox. A streaming
scenario initially expected discovery text that the adopted-session status
replaces; it now checks the actual worktree list and continuing session behavior.

The full run also exposed an inherited screen test waiting for transient line
0000 despite 5 ms output and a 50 ms dashboard poll. It now waits for a visible
stream prefix while preserving its final-line and exact public output-byte
boundary assertions. No screen history or timeout was enlarged.

Reproduction, successful after merging styled screen/resize integration e5eae3c:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

The test suite includes 16 unit tests plus one ignored probe driven by a passing
terminal-restoration test, 11 ownership/recovery tests, one scripted-session test
and one styled-screen/resize integration test. These checks do not establish
history durability, storage-fault behavior, supervisor-loss cleanup, native CLI
support, arbitrary terminal-app behavior or measured product latency targets.
