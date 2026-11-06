# 03: Preserve native input and focus

Status: ready-for-agent
Blocked by: 01
Resolved: yes

**What to build:** Switch explicitly between dashboard navigation and native-session interaction without stealing agent shortcuts or accidentally stopping work.

- [x] Dashboard and attached focus are visible in the pane and help text. Navigation keys operate the dashboard only in dashboard focus; attached input reaches the child.
- [x] Ctrl-C in attached focus retains native interrupt behavior and does not become global quit. Choose, document and display an escape chord that returns to dashboard focus without terminating the child.
- [x] Forward Unicode, paste including multiline/bracketed paste where applicable, normal shortcuts and modifier combinations with demonstrated byte behavior. Handle split UTF-8 sequences without corrupting input or output.
- [x] The scripted PTY child verifies received bytes, native-style prompt answers, interruption, escape-to-dashboard and return to the same child. Extend existing state/event tests for focus transitions without duplicating integration tests at every internal module.
- [x] PTY/native-terminal smoke checks verify restoration of raw mode, alternate screen and any enabled input modes after normal exit, errors and Rust panic.
- [x] Keep encoding, PTY writes and other blocking effects outside the UI loop. Document known terminal/input limitations; final escape-chord compatibility with both real tools is checked by their native proof tickets.


## Comments

Implemented and verified 2026-11-06. The macOS real dashboard/supervisor/scripted
PTY test verifies split UTF-8, bracketed multiline paste, exact modifier/shortcut
bytes, native-style prompt answers, Ctrl-C byte/fixture interruption, Ctrl-]/Ctrl-5
focus escape and reattach to the same PID. Application and rendering tests cover
focus, modifier-sensitive dashboard actions, limits and visible help. A real PTY
subprocess probe compares complete termios and alternate-screen/paste-mode
sequences after normal return, returned error and Rust panic. See
[byte receipts and limits](../../../docs/proofs/03-native-input.md).

Stable `cargo build --locked`, `cargo fmt --all -- --check`,
`cargo clippy --locked --all-targets -- -D warnings`, and escalated
`cargo test --locked --all-targets` pass (11 unit tests plus dashboard integration;
one ignored subprocess probe is exercised three times by the restoration test).
The sandbox denies supervisor socket creation; integration ran outside it.
No real Claude Code/Codex support, native interrupt lifecycle, enhanced keyboard,
mouse or terminal-emulator-app compatibility claim follows. Both native proof
tickets retain final escape/input compatibility checks.
