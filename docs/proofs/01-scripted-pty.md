# Scripted PTY first slice — 2026-11-06

This record covers ticket 01 only. It proves fixture functionality on macOS,
not support for Claude Code or Codex CLI. Neither native tool was launched.

## Environment and reproduction

- macOS 26.2 (25C56), arm64; rustc 1.99.1 stable.
- Debug build; portable-pty 0.9.0 and vt100 0.18.1.
- Dashboard exercised through an actual 120×30 PTY using the same executable's
  supervisor mode and scripted-child mode. No external terminal emulator was
  involved: vt100 parses the dashboard terminal output for assertions.
- An isolated temporary Git repository with spaces in its path and isolated
  private state/socket directory under `/private/tmp` prevents connection to a
  developer supervisor. Test cleanup kills/reaps its dashboard and requests
  shutdown through only that isolated endpoint, including assertion failures.

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

The test requires permission to bind Unix sockets and create PTYs. The execution
sandbox denied socket binding with EPERM; the successful macOS run used approval
to execute the test outside that sandbox.

## Results

All commands passed. The full suite passed seven existing tests plus the new
`dashboard_adopts_launches_and_interacts_without_discovery_mutation` integration
test. The initial complete fixture run finished in 0.82 seconds (test duration,
not a navigation/attach latency measurement).

The dashboard test observes read-only discovery with no adopted checkout or child,
explicit adoption, a real fixture prompt in the selected canonical cwd, a visible
UUID task identity, submitted input and response, dashboard escape, same-session
reattach and another response, then successful dashboard exit after supervisor
cleanup. Git's working status remains unchanged; the supervisor removes its
isolated endpoint only after killing and waiting for the fixture. A process-list
check after failed-test cleanup found no remaining prototype processes.

The integration test was authored before the session implementation; its initial
compile attempt exposed the parser's nonzero-dimension API requirement. The
subsequent behavior run exposed a real input failure: raw Ctrl-] is reported by Crossterm as Ctrl-5. Recognizing both
representations made the escape test pass. A failed socket connection under the
sandbox was diagnosed separately from application behavior. No native support
claim follows from these fixture results.

The reference image was inspected. The implementation follows reviews above the
project/worktree/session tree on the left and a large bordered session pane on
the right, with dark background, cyan borders, focus/status labels and working
keyboard help. Reviews show unavailable. There is one fixture session and a
parsed child screen rather than the image's message composer. Long paths wrap
in selection details. Existing rendering tests also exercise 12×4 resizing;
usable small-screen interaction and screenshot comparisons remain later proof.

## Initial caps and limits

- Parser: fixed 100×24 cells, no scrollback; PTY output processed in 4 KiB chunks.
- Dashboard transport: 32 queued commands, one queued coherent screen snapshot.
- Protocol: 64 KiB frame maximum; 4 KiB input maximum. Full command queues return
  an explicit retryable error. Slow snapshot consumers resynchronize on a later
  snapshot. Screen output never becomes an unbounded event transcript.
- UI handles at most 32 Git observations and one bounded session snapshot per
  frame. Blocking sockets, Git, files, spawn, reads/writes and reaping use workers.
- Fixed PTY size can crop/wrap relative to the visible pane; viewport resize,
  terminal cell styles, paste, mouse, function/arrow keys and full Unicode/control
  encoding require follow-on input/geometry work. Ctrl-] is provisional.
- IDs are UUIDs stable for the supervisor lifetime only. No metadata/transcript
  persistence, owner election, launch reconciliation, robust second-controller
  rejection, bounded stop escalation or descendant cleanup is claimed.
- No transcript is captured. The last screen is ephemeral and is not history.
  Failed/disconnected transport is visible. Unexpected dashboard loss can leave
  fixture execution alive; reconnect and ordinary quit clean it up. This has not
  established crash recovery guarantees.
- No output-rate, memory, latency, storage, native-status, native-CLI or terminal
  restoration compatibility proof has been performed here. Later milestone
  tickets retain those gates.
