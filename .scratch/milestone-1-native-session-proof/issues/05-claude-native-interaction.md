# 05: Demonstrate Claude Code interaction on macOS

Status: ready-for-agent
Blocked by: 04

**What to build:** Use real Claude Code through the dashboard’s native PTY interaction in an explicitly adopted checkout, with the developer’s existing authentication.

- [ ] Expose a working Claude Code launch choice and launch directly in the adopted checkout. Missing executable, authentication problems and spawn failure show actionable errors without losing context or spawning duplicate attempts.
- [ ] On macOS, answer a real native input prompt and approval prompt, exercise native Ctrl-C, complete a turn, return using the documented escape chord and reattach to the same conversation/process.
- [ ] Repeated resizing, Unicode/paste and relevant native terminal modes remain usable. Resolve escape-chord conflicts explicitly rather than silently replacing native shortcuts.
- [ ] Retain discovery, selection and navigation while Claude Code runs. Use a safe temporary checkout for the native demonstration; do not copy credentials into product state or the evidence record.
- [ ] Record exact CLI, macOS and terminal versions, reproducible steps, results and compatibility limitations. Fixture results and real macOS evidence are clearly distinguished; status-signal reliability is established in its separate ticket.
- [ ] An unavailable or failed native scenario remains an explicit proof failure requiring resolution; do not advertise established support based only on launch or fixture success. Update working launch/input instructions.
