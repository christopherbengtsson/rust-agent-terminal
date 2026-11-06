# Agent terminal

A Rust terminal workspace for coding agents. The product name is undecided;
`agent-terminal` is the current executable name. See the [vision](VISION.md),
[product requirements](PRODUCT.md), [architecture](docs/architecture.md), and
[implementation milestones](docs/milestones.md).

The dashboard discovers real Git worktrees, explicitly adopts a checkout, and
launches Claude Code (`c`), Codex CLI (`x`), or a scripted PTY fixture (`l`). Native
launch is experimental; see the recorded version-specific proofs below. Durable
readable history and supervisor reconnect are implemented; full crash
reconciliation, semantic status and GitHub workflows remain later gates. The first
release targets macOS; fixture tests do not establish native-agent compatibility.

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
| `c` | Launch installed Claude Code directly in the adopted checkout (experimental) |
| `x` | Launch installed Codex CLI with `--no-daemon` in the adopted checkout (experimental) |
| `l` | Launch the scripted PTY fixture in the adopted checkout |
| Enter | Attach to the existing managed session |
| Ctrl-] | Return from attached focus to dashboard navigation |
| `s` | In dashboard focus, stop the fixture and keep the dashboard open |
| `d` | In dashboard focus, detach and exit while preserving execution |
| `q` / Esc / Ctrl-C | In dashboard focus, request supervisor shutdown and quit after confirmed stop |

In attached focus, UTF-8 characters, Ctrl-C, legacy control/Alt shortcuts,
arrows, navigation keys, F1–F12 and Shift-Tab reach the fixture PTY. The pane and
help show input focus; attached `q` and Esc are native input. Ctrl-] (the same
legacy byte as Ctrl-5) returns to dashboard focus without stopping the fixture;
Enter reattaches to the same child. This provisional chord still needs both
native-agent proofs. Ordinary quit visibly waits for the supervisor's confirmed
stop. Stop sends TERM to the managed process group, escalates to KILL after
500 ms, then confirms the leader exit and absence of executing group members.
Verification errors or execution remaining after the two-second deadline stay
visible; retry `s` or `q` after addressing the error. A verification subprocess
has its own 500 ms timeout. These are fixture-tested process-group boundaries;
processes that deliberately leave that group are outside this cleanup guarantee.
Native `exit` retains the latest screen and task/session attribution and returns
to dashboard focus. A subsequent explicit launch creates a new session.
Neither native exit nor stop deletes working files. Detach restores the terminal
and closes only the dashboard connection. Unexpected dashboard loss also preserves
execution. No durable-history flush is implemented; that remains ticket 08.

Bracketed paste is enabled on the outer terminal and forwarded with delimiters
when the child requests that mode; otherwise multiline paste normalizes line
breaks to Enter. A paste over 4084 UTF-8 bytes is visibly rejected for retry in
smaller pieces. The scripted child reports exact bytes, accepts `approve` then
`y`/`n`, handles Ctrl-C as an interrupt, and `identity` reports its PID.

Encoding uses legacy xterm sequences. Super/Hyper/Meta and unmappable control
characters report an error. Legacy terminals cannot distinguish every modifier
combination (for example Ctrl-Enter versus Enter), or Ctrl-] versus Ctrl-5.
Keyboard enhancement protocols, mouse forwarding and IME composition guarantees
remain unimplemented. Child cursor-key mode is respected by the supervisor at encoding time. Paste requires terminal bracketed-paste support; Crossterm buffers an entire
incoming paste before the application can reject its size. See the
[input proof](docs/proofs/03-native-input.md) for the tested bytes and boundaries.

The supervisor runs as another mode of the same executable, with a private local
socket under `~/Library/Application Support/rat/proof`. For isolated experiments:

```sh
cargo run --locked -- /path/to/repository --state-dir /tmp/rat-proof-example
```

One dashboard controls the supervisor at a time. A second dashboard reports the
existing connection and cannot launch, type into, or stop its session. After a
controller dies, reopen the dashboard to recover the same fixture and screen.
Symlink checkout aliases and paths inside a checkout adopt its canonical Git
worktree root. A new launch while running is refused; Enter attaches to the existing
child. The surviving supervisor retains launch-request outcomes: reconnect
recovers the original request and its success or failure, and retrying that exact
request does not execute again. An explicit `c` or `l` after an acknowledged outcome is
a new request. This recovery applies to dashboard loss; supervisor loss needs the
separate crash-reconciliation work. See the [reconnect proof](docs/proofs/07-dashboard-reconnect.md).

The state directory must be owned by the current user with mode 0700 and cannot
itself be a symlink. The socket is mode 0600. Concurrent starts use an OS ownership
lock; only its owner can remove a stale, refused socket. A live or uncertain
endpoint is never displaced. Keep the lock file in place, including after shutdown.
Use separate private `--state-dir` directories only for isolated experiments.

This prototype has no durable metadata/history, supervisor-crash launch
reconciliation, or cleanup of processes that escape the managed process group. Do not use it for real coding
work. Dashboard loss preserves the supervisor and fixture; reconnect and quit
normally to clean up. Do not remove a socket for a running supervisor. The bounded
parsed screen follows the session pane viewport (up to 200×80 cells), without
scrollback; it is not history. The PTY and parser resize together, including
while dashboard focus is active. Small terminals hide the sidebar and retain
a short session identity, focus/status and essential actions. Native cell colors,
styles, wide Unicode and cursor are rendered within the pane; child terminal
controls never run directly on the outer terminal. Mouse requests are parsed but
mouse input is not forwarded. See the [screen proof](docs/proofs/04-session-screen.md)
for geometry, captures and parser limitations.
See the [first-slice evidence](docs/proofs/01-scripted-pty.md) and
[ownership evidence](docs/proofs/02-session-ownership.md) and
[lifecycle evidence](docs/proofs/06-lifecycle.md) for tested scope.

## Development

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

CI runs these checks on pull requests on macOS and Linux. Tests create isolated
temporary repositories and do not require agent accounts or GitHub access.

Claude Code launch uses `claude` from the supervisor's inherited PATH and the
existing native login. Install Claude Code and run `claude auth login` in your
usual terminal if missing or unauthenticated, then reopen the dashboard if PATH
changed. Authentication/setup prompts remain in the native CLI. A failed spawn
retains the adopted checkout and discovery; `c` is an explicit retry. A running
session refuses replacement regardless of the selected launch choice. Launch is
experimental: see [the native proof](docs/proofs/05-claude-native.md) for the exact
version and unresolved scenarios. Native tool usage can consume your account's
allowance. No credential copying or product authentication store is used.

Codex CLI launch uses `codex --no-daemon` from the supervisor's inherited PATH,
keeping the managed interactive runtime outside Codex's shared daemon. It reuses
existing native authentication; run `codex login` in your usual terminal if
needed. Native authentication/setup prompts stay in the CLI. Failed spawn keeps
the adopted checkout; `x` explicitly retries. Stop the current session before
changing tools: launching the other tool starts a new conversation and does not
transfer context. See [the Codex native proof](docs/proofs/11-codex-native.md)
for the tested version and limitations. Semantic status remains unknown pending
the separate native status proof.

Readable history is independent of a live child, native resume, and the checkout.
In dashboard focus press `h` to open/close retained history, `n`/`p` for older/newer
sessions, `[`/`]` for byte pages, and `j`/`k` to scroll the current page. History
shows chronological captured output; carriage returns and escape controls appear
as printable escapes, including text erased by redraws or alternate screens.
Input keys/paste are not separately recorded, so hidden password input is not
copied into history. Output itself can contain sensitive text.

The supervisor stores SQLite metadata and 64 KiB transcript chunks under
`~/Library/Application Support/rat/proof` (or the explicit isolated `--state-dir`).
Metadata is marked unconfirmed until its commit succeeds. Ordinary quit waits
for process cleanup, PTY EOF, transcript file synchronization and the metadata
commit. A failed write keeps quit visibly incomplete; repair storage and retry
`q`. There is no automatic history deletion. Supervisor loss restores interrupted
records without starting coding; unresolved execution blocks replacement launch.
An abrupt failure can lose output beyond the committed prefix. See
[the history proof](docs/proofs/08-durable-history.md) for tested boundaries and
preliminary healthy-storage observations.
