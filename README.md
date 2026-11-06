# Agent terminal

A Rust terminal workspace for coding agents. The product name is undecided;
`agent-terminal` is the current executable name. See the [vision](VISION.md),
[product requirements](PRODUCT.md), [architecture](docs/architecture.md), and
[implementation milestones](docs/milestones.md).

The dashboard discovers real Git worktrees and can explicitly adopt a checkout
and launch an interactive scripted PTY fixture. This is a supervisor/terminal
prototype: Claude Code, Codex CLI, durable history, recovery, and GitHub workflows
are not supported yet. The first release targets macOS; fixture tests do not
establish native-agent compatibility.

## Run

Install stable Rust and Git, then run from an interactive terminal:

```sh
cargo run --locked
cargo run --locked -- /path/to/repository
```

With no argument, the dashboard inspects the current directory. You can point
it at any directory inside a repository or one of its linked worktrees.
Discovery reads Git metadata; it does not create worktrees or start agents.
Errors appear in the dashboard, where you can retry or quit.

| Key | Action |
| --- | --- |
| `j` / Down | Select next worktree |
| `k` / Up | Select previous worktree |
| `r` | Refresh worktrees in the background |
| `a` | Explicitly adopt the selected usable checkout |
| `l` | Launch the scripted PTY fixture in the adopted checkout |
| Enter | Attach to the existing fixture |
| Ctrl-] | Return from attached focus to dashboard navigation |
| `q` / Esc / Ctrl-C | In dashboard focus, stop the fixture and quit |

In attached focus, UTF-8 characters, Ctrl-C, legacy control/Alt shortcuts,
arrows, navigation keys, F1–F12 and Shift-Tab reach the fixture PTY. The pane and
help show input focus; attached `q` and Esc are native input. Ctrl-] (the same
legacy byte as Ctrl-5) returns to dashboard focus without stopping the fixture;
Enter reattaches to the same child. This provisional chord still needs both
native-agent proofs. Ordinary quit waits for the supervisor's confirmed stop.

Bracketed paste is enabled on the outer terminal and forwarded with delimiters
when the child requests that mode; otherwise multiline paste normalizes line
breaks to Enter. A paste over 4084 UTF-8 bytes is visibly rejected for retry in
smaller pieces. The scripted child reports exact bytes, accepts `approve` then
`y`/`n`, handles Ctrl-C as an interrupt, and `identity` reports its PID.

Encoding uses legacy xterm sequences. Super/Hyper/Meta and unmappable control
characters report an error. Legacy terminals cannot distinguish every modifier
combination (for example Ctrl-Enter versus Enter), or Ctrl-] versus Ctrl-5.
Keyboard enhancement protocols, mouse forwarding, IME composition guarantees and
dynamic PTY resize remain unimplemented. Child cursor-key mode is respected by the supervisor at encoding time. Paste requires terminal bracketed-paste support; Crossterm buffers an entire
incoming paste before the application can reject its size. See the
[input proof](docs/proofs/03-native-input.md) for the tested bytes and boundaries.

The supervisor runs as another mode of the same executable, with a private local
socket under `~/Library/Application Support/rat/proof`. For isolated experiments:

```sh
cargo run --locked -- /path/to/repository --state-dir /tmp/rat-proof-example
```

Use one dashboard per isolated state directory. This prototype has no durable
metadata/history, owner election, stale-endpoint recovery, or guaranteed descendant
cleanup. Do not use it to run real coding work. A lost dashboard may leave its
supervisor and fixture running; reconnect to that state directory and quit normally
to clean up. Do not remove a socket for a running supervisor. The bounded parsed
screen retains the latest 100×24 cells without scrollback; it is not session history.
See the [prototype evidence](docs/proofs/01-scripted-pty.md) for measured scope.

## Development

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

CI runs these checks on pull requests on macOS and Linux. Tests create isolated
temporary repositories and do not require agent accounts or GitHub access.
