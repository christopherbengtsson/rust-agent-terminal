# Session ownership — 2026-11-06

Ticket 02 extends the scripted dashboard proof. It establishes fixture ownership
on macOS 26.2 (25C56), arm64, stable rustc 1.99.1, debug build, portable-pty 0.9.0
and vt100 0.18.1. Neither Claude Code nor Codex CLI was launched.

## Reproduction and observations

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

The integration tests use actual 160×40 dashboard PTYs, supervisor processes,
scripted children and temporary real Git repositories under `/private/tmp`.
Each scenario uses a separate private state directory; cleanup addresses only
that isolated endpoint. Unix socket and PTY creation require execution outside
the sandbox. All commands above passed; the ownership scenarios took 1.47 seconds
in the initial complete passing run (test duration, not latency evidence).

- Two supervisor processes race against a stale bound-and-closed socket. Exactly
  one contender survives and the dashboard adopts and launches successfully.
  Process-list observations scoped to the endpoint establish one owner and one
  managed child. Directory and socket permissions are respectively 0700 and 0600.
- A live controller launches a fixture. A second dashboard displays the existing
  controller conflict; its adoption, launch and quit keys cannot affect the
  fixture. The first controller still answers a prompt. The process list shows
  exactly one supervisor and fixture.
- Killing the dashboard and reopening recovers the same visible child PID and
  screen; the replacement answers another prompt and quits normally. This caught
  and fixed a real SIGHUP issue: supervisor startup now establishes its own OS
  session before exec.
- Opening through a symlink to a checkout subdirectory launches in the canonical
  Git worktree root. Repeated adoption/launch reports an existing session; attach
  retains the same fixture PID and further input works. One owner/child remains.
- A symlink state directory fails with an actionable private-directory error,
  preserves the rendered discovered worktree, and creates no endpoint in the
  symlink target. State tests also preserve checkout selection and navigation on
  control refusal. Existing tests continue to cover distinct linked worktrees,
  bare/prunable discovery and non-UTF-8 paths.

The first refusal test was written before implementation. Its approved execution
failed because the second dashboard timed out instead of explaining ownership.
The implementation then passed it. An earlier sandbox execution could not bind
its endpoint; that environmental failure was rerun with approval. The extended
private-directory test initially expected the generic discovery status even when
the specific ownership error correctly occupied it; its expectation now checks
the retained worktree list and error. Clippy identified and fixed a needless test
borrow. No native compatibility claim follows from any fixture result.

## Boundaries

The OS lock remains on disk after shutdown; only its held kernel lock establishes
ownership. Never unlink the lock while a supervisor may be alive. Only the lock
owner recovers a socket with an explicitly refused connection. Live/uncertain
endpoints and unexpected endpoint types fail closed. Unexpected client loss
preserves the running child and bounded parsed screen, and EOF releases control.

This prototype still owns only one child total. Distinct worktree concurrency,
durable metadata/history, launch acknowledgement recovery, supervisor-crash
survivor reconciliation, native resume, bounded descendant cleanup, native-tool
compatibility and performance/resource measurements belong to later tickets.
A supervisor crash is outside this ownership proof; a lock alone cannot establish
whether an already-launched child survived. Do not use the fixture for coding work.
