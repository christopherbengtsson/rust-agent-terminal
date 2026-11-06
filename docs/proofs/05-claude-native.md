# Ticket 05: Claude Code native interaction

2026-11-06, macOS 26.2 (25C56), arm64. The real CLI session displayed
Claude Code **2.1.318**, Haiku 5.5 medium effort. Its canonical installed binary
`~/.local/share/claude/versions/2.1.318` has SHA-256
`677a2ea7bf576cedd20e191d973914ed8a41796f53a63b1923381ddc4d5b5e7d`.
Claude auto-updated the installed symlink to 2.1.319 during the experiment; the
running session continued displaying 2.1.318 and “Restart to update”. The
symlink hash `1f53587e92dee8e089fa349188a4cda64fa34ace6e7b0ca2ee278ddaa3cc2ebd`
is **not** the tested session binary hash. No updater was invoked by this proof.
The older 2.1.74 research is not this native evidence.

## Terminal and execution context

The experiment used real macOS PTYs driven by Python 3.14.3 (`pty`, `termios`,
`TIOCSWINSZ`), a real dashboard and separate supervisor, portable-pty 0.9.0,
Crossterm 0.29.0, vt100 0.18.1 and Ratatui 0.30.2. Child TERM is
`xterm-256color`. There was no graphical terminal-emulator application, so no
Terminal.app/iTerm version or compatibility is claimed. Initial outer geometry
was 160×40, with a 104×32 child pane. Recorded text files are parsed child
screen captures, not terminal-app screenshots. Native sockets/PTYs ran outside
the execution sandbox after its socket restrictions prevented supervisor startup.

`claude auth status` was projected to loggedIn=true only; no credential values
or account identity were recorded. Launch reused that existing authentication.
The isolated Git checkout was `/private/tmp/rat-claude-proof-checkout`, and the
isolated supervisor state was `/private/tmp/rat-claude-native-state`. Native
folder trust was explicitly accepted for this empty checkout. Claude's native
session/trust metadata was allowed; no global/repository policy configuration
was edited, no credentials were copied, and no hooks were injected.

## Reproduce

1. `cargo build --locked`, initialize a fresh temporary Git checkout, and run
   `target/debug/agent-terminal /path/to/temporary-checkout --state-dir /path/to/private-proof-state`
   in a 160×40 terminal/PTY. Do not use a real working project for this proof.
2. Wait for discovery, press `a`, then `c`. Discovery alone must not launch.
   In Claude's folder-trust prompt, select “Yes, I trust this folder”.
3. Paste `Reply exactly: native-proof λ 界. Do not use tools or write files.`
   using bracketed paste, then Enter. Observe `native-proof λ 界` and Claude's
   native done marker. [Captured screen](screens/05-claude-turn.txt).
4. Ctrl-] returns to dashboard; `d` exits without stopping the child. Reopen
   against the same state directory. The same task/session/request identity and
   conversation remain. Each following step used this escape/detach/reopen path;
   it did not issue another launch. Enter reattaches if dashboard focus is active.
5. The existing native mode was auto; Shift-Tab selected manual mode for the
   approval demonstration. Paste `Use AskUserQuestion to ask me whether the proof
   marker should be alpha or beta. Do not write files or run commands.` and Enter.
   Claude displayed a real native multiple-choice question; Enter selected alpha.
   Its answer confirmed alpha and completed the turn.
   [Question screen](screens/05-claude-question.txt).
6. Request `Run exactly this Bash command in this temporary proof checkout:
   printf approval-proof > approval-marker. Do not write outside this checkout
   or run other commands.` The native approval screen showed the exact command
   and Yes/always-allow/auto/No choices. [Approval screen](screens/05-claude-approval.txt).
   A preceding `printf approval-proof` without redirection ran under existing
   permissions without a prompt; that was not counted as approval evidence.
7. While the approval prompt was active, resize the outer PTY through 100×24,
   60×18, 160×40, 120×28, 160×40 with SIGWINCH. The native prompt remained
   usable. Enter approved **once**, without selecting always-allow or auto mode.
   Claude completed; the only checkout change was untracked `approval-marker`
   containing exactly `approval-proof`.
   [Completed/resized screen](screens/05-claude-approved-resized.txt).
8. Request `Count from 1 to 1000, one number per line. Do not use tools.`
   Send Ctrl-C after generation begins. Claude stopped at 204 and displayed
   `Interrupted · What should Claude do instead?`; the dashboard remained alive.
   [Interrupted screen](screens/05-claude-interrupted.txt).
9. Native `/exit` stopped the CLI. The supervisor observed Process Exited,
   retaining adopted path, agent label, task/session/request identity and screen.
   Native resume guidance was visible, but native resume was not tested here.

Observed final managed identity was task
`97151497-7ecf-4fab-8d3c-beabe7ffea94`, session
`3842d941-5019-436c-8a0c-f29b700e9abe`, request
`f2ee42f1-95c7-4ad5-bffd-750c88e0f30a`, with 54,593 parsed output bytes and
104×32 child geometry. No replacement coding child was started across reopens.

## Results and limits

Real native input, a multiple-choice question/answer, approval/one-time answer,
Unicode bracketed paste, completed turns, active Ctrl-C, escape/reattach,
detach/reconnect, repeated resize, and native exit passed for this 2.1.318
session. The parsed approval/question screens remained coherent. No Ctrl-]
conflict appeared in these workflows; it reserves the same legacy byte as Ctrl-5.
Mouse forwarding, enhanced keyboard protocols, IME, graphical terminal apps,
native resume, long-running workloads and other tool versions remain unverified.
Semantic status remains explicitly unknown; visual native evidence here does not
implement the per-event status integration in ticket 10.

The user's existing SessionStart hook reported a nonblocking error:
`${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.cmd` was unavailable. Native interaction
continued. The proof did not repair or disable that hook. Future status work must
resolve/observe this environment and cannot infer successful hook delivery.
Authentication-error paths were not forced by logging out or changing credentials;
login guidance is in `--help`/README and Claude retains its native authentication
UI. No automatic spawn retry or credential preflight occurs in the UI loop.

## Fixture verification

An App test first failed to compile before LaunchClaude existed, then passed for
explicit `c` versus `l` selection. The real outer-dashboard missing-executable
scenario restricts PATH to Git/ps, observes actionable Claude launch guidance and
retained adoption, then successfully launches the fixture. Failed launch replay
is checked for both fixture and Claude request variants without executing a native
model. Native observations above are separate from these fixture tests.

Ticket 07's immediate reopen tests exposed asynchronous controller-EOF release:
a dashboard could legitimately receive the existing-controller refusal before the
supervisor consumed EOF. The test harness now retries only after that public UI
refusal, bounded by eight seconds; it does not change product ownership policy.
There is no fixed release sleep. The supervisor polls on a five-millisecond
idle interval, which is an implementation interval rather than a latency promise.
EOF detection/release is proved by eventually obtaining the retained screen.

Final stable toolchain: rustc 1.99.1 (58cf8f9c3 2026-10-13), cargo 1.99.1
(5f94df478 2026-08-27). `cargo build --locked`, `cargo fmt --all -- --check`,
`cargo clippy --locked --all-targets -- -D warnings`, and
`cargo test --locked --all-targets` passed after merging integration commit
2d4d74b (tickets 04, 06 and 07): 17 unit tests passed, one ignored subprocess
probe driven by the passing restoration test; 12 ownership scenarios passed in
17.37s; scripted interaction passed in 3.52s; screen proof passed in 8.70s.
These runtimes are not performance claims. The isolated native proof supervisor
was explicitly shut down after Claude exited, with confirmed stopped=true.
