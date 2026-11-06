# 03: Preserve native input and focus

Status: ready-for-agent
Blocked by: 01

**What to build:** Switch explicitly between dashboard navigation and native-session interaction without stealing agent shortcuts or accidentally stopping work.

- [ ] Dashboard and attached focus are visible in the pane and help text. Navigation keys operate the dashboard only in dashboard focus; attached input reaches the child.
- [ ] Ctrl-C in attached focus retains native interrupt behavior and does not become global quit. Choose, document and display an escape chord that returns to dashboard focus without terminating the child.
- [ ] Forward Unicode, paste including multiline/bracketed paste where applicable, normal shortcuts and modifier combinations with demonstrated byte behavior. Handle split UTF-8 sequences without corrupting input or output.
- [ ] The scripted PTY child verifies received bytes, native-style prompt answers, interruption, escape-to-dashboard and return to the same child. Extend existing state/event tests for focus transitions without duplicating integration tests at every internal module.
- [ ] PTY/native-terminal smoke checks verify restoration of raw mode, alternate screen and any enabled input modes after normal exit, errors and Rust panic.
- [ ] Keep encoding, PTY writes and other blocking effects outside the UI loop. Document known terminal/input limitations; final escape-chord compatibility with both real tools is checked by their native proof tickets.
