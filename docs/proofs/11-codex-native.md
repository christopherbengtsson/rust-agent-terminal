# Ticket 11: Codex CLI native interaction

2026-11-06, macOS 26.2 (25C56), arm64, Codex CLI **0.167.0**.
Canonical installed binary:
`~/.codex/packages/standalone/releases/0.167.0-aarch64-apple-darwin/bin/codex`.
SHA-256: `030316ecfbb41f4e7946e584b0063ef333b7a36cd312ba349f2f6869e39b62cc`.
The real native screen identified v0.167.0 and GPT-6.1-Sol medium.

## Execution and reproducible steps

Real macOS PTYs were driven with Python 3.14.3 `pty`, `termios` and
`TIOCSWINSZ`, using the real debug-build dashboard and separate supervisor.
Stable Rust/cargo 1.99.1, portable-pty 0.9.0, Crossterm 0.29.0, vt100 0.18.1
and Ratatui 0.30.2 were used. Child TERM was `xterm-256color`. Initial outer
geometry was 160×40 and child pane 104×32. There was no graphical terminal app;
these text captures are parsed child screens, not Terminal.app screenshots.
Native socket/PTY operations ran outside the execution sandbox after sandboxed
startup failed with unresolved supervisor ownership (Unix socket restriction).

1. Build with `cargo build --locked`. Initialize an empty temporary Git checkout
   (`git init -b main /private/tmp/rat-codex-proof-checkout`). Run
   `target/debug/agent-terminal /private/tmp/rat-codex-proof-checkout --state-dir /private/tmp/rat-codex-native-state`
   in the declared PTY. Discovery launches nothing. Press `a`, then `x`.
2. Codex displayed its native folder-trust question. Accept “Trust and continue”
   for this empty checkout. The product launches `codex --no-daemon` directly with
   cwd set to this checkout. Existing ChatGPT login was reused. No credentials
   were copied into product records; no global/repository policy configuration
   was edited, no bypass flags or monitoring hooks were injected. Codex's normal
   native trust/session metadata was allowed.
3. Bracketed-paste `Reply exactly: native-proof λ 界. Do not use tools or write files.`
   and Enter, then repeat with `codex-proof λ 界`. Both replies preserved Unicode
   and completed with native “Worked for” markers.
   [Completed turns](screens/11-codex-turn.txt).
4. Ctrl-] returns to dashboard without stopping Codex; `d` detaches. Reopen the
   same dashboard/state path and attach with Enter when necessary. This
   escape/detach/reopen sequence was repeated between every following step.
   It never pressed another launch key. The same task/session/request and native
   conversation remained throughout.
5. Paste `Ask me to choose alpha or beta and wait for my answer. Do not use tools or write files.`
   Codex asked “Choose alpha or beta.” [Question](screens/11-codex-question.txt).
   Answer `alpha. Acknowledge my choice, then run exactly printf approval-proof > approval-marker in this proof checkout using require_escalated permission with justification so I can approve it once. Do not run other commands or write other files.`
   Codex acknowledged alpha and displayed a real native command approval with
   Yes/proceed, persistent-prefix permission and No choices.
   [Approval](screens/11-codex-approval.txt). This was a conversational input
   question, not a dedicated multiple-choice input tool or semantic status signal.
6. While approval was pending, resize the outer PTY through 100×24, 60×18,
   160×40, 120×28, 160×40 using `TIOCSWINSZ` plus SIGWINCH. The prompt remained
   operable. Enter selected the one-time approval. Codex confirmed “You approved
   codex ... this time”, ran the exact command, and completed the turn. The sole
   checkout change was untracked `approval-marker` containing `approval-proof`.
   [Resized/approved screen](screens/11-codex-approved-resized.txt).
7. Send `Count from 1 to 1000, one number per line. Do not use tools.`, Enter,
   then Ctrl-C during the active turn. Codex displayed “Conversation interrupted”;
   the dashboard and same process remained alive.
   [Interrupted screen](screens/11-codex-interrupted.txt). The interrupt arrived
   before visible counting output; it proves active-turn cancellation, not a
   long-running command/descendant cancellation workload.
8. Native `?` opened keyboard shortcuts, Esc closed them, and `/exit` exited
   Codex. [Shortcuts](screens/11-codex-shortcuts.txt). Supervisor observed
   Lifecycle Exited while retaining agent, adoption and identity. Exit displayed
   `codex resume 01a1b0ef-661e-7b61-bc9a-335a4c88596f`; native resume was not
   exercised and is not claimed here. The isolated supervisor was then shut down
   explicitly, with `stopped=true` acknowledged.

## Ownership, modes and scope

All captures retained task `8690aa9a-ac42-4e61-93b4-2152968bf310`, session
`47bd039a-fd75-475a-ad40-d24673e4d345`, request
`10a943f1-d58a-4b0a-9bba-e812c61d27eb`. Final output boundary was 75,945 bytes.
Process inspection recorded supervisor PID 94028 (own group), direct child PID
94030/PGID 94030 running `codex --no-daemon`. Across reconnects the child PID
remained 94030; after native exit it was absent. No shared daemon was launched
for this invocation. This validates this direct session's native-exit cleanup;
it does not prove every possible tool descendant's behavior.

The parsed native state enabled bracketed paste (`CSI ?2004h`) and mouse modes
1003/1006. Unicode paste and keyboard interaction passed; mouse events are not
forwarded and mouse compatibility is unverified. No alternate-screen mode was
active in the captured Codex state. Enhanced keyboard protocols, IME, terminal
apps and other Codex versions remain unverified. Ctrl-] (legacy Ctrl-5 byte) was
reserved for dashboard focus and no conflict appeared in these workflows;
ordinary `?`, Esc and active Ctrl-C retained native behavior. The product uses a
native pane, rather than a replacement composer, and native status stays unknown.
Reliable semantic status is a separate ticket; visual completion/approval here
must not be interpreted as a semantic adapter implementation.

Installed `codex --help` and `codex login status` were checked first; auth reported
only “Logged in using ChatGPT”. The [official command reference](https://learn.chatgpt.com/docs/developer-commands#global-flags)
was searched and fetched to corroborate `--no-daemon`. Existing configuration,
authentication and account policy were preserved. Authentication failure was not
forced by logging out or altering credentials; native authentication UI and
`codex login` guidance remain available. Unsupported versions without
`--no-daemon` will show native CLI argument errors rather than silently using a
shared daemon. Changing tool choice requires stopping the existing session and
starts a fresh conversation; no context transfer is implied.

## Fixture checks and implementation verification

The approved App seam test was written first and failed to compile because
LaunchCodex did not exist, then passed after implementation. The outer-dashboard
PTY test restricts PATH to Git/ps, presses `x`, observes actionable missing/spawn
failure guidance with retained adoption, then launches the fixture successfully.
Failed launch replay now covers Launch, LaunchClaude and LaunchCodex, proving
that known failed requests do not execute after later adoption. Fixture evidence
is separate from the native observations above.

A render regression exposed the added choice pushing quit guidance beyond 120
columns; the help was shortened and the same render test passed with both Codex
and quit/stop visible. Required locked build, formatting, warnings-denied Clippy
and all-target tests passed before final integration (18 unit tests plus one
ignored subprocess probe driven by terminal restoration, 13 ownership scenarios,
scripted interaction and styled-screen proof). Final integration verification is
recorded below.

Final integration: merged `b722dfd` (ticket 08 durable history) into the ticket 11
branch and included LaunchCodex in the save trigger, shared retained request map
and interrupted-execution guard. Agent labels flow through the existing durable
metadata. `cargo build --locked`, `cargo fmt --all -- --check`,
`cargo clippy --locked --all-targets -- -D warnings` and
`cargo test --locked --all-targets` passed: 18 unit tests, one ignored subprocess
probe exercised by the passing restoration test, 20 ownership scenarios (36.08s),
scripted interaction (3.64s), and screen proof. The final native run itself was
before this persistence merge; the native launch/input/process path was preserved,
and merged behavior was checked through the real dashboard/supervisor seam.
