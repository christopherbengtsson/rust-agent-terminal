# 05: Demonstrate Claude Code interaction on macOS

Status: ready-for-agent
Blocked by: 04
Resolved: yes

**What to build:** Use real Claude Code through the dashboard’s native PTY interaction in an explicitly adopted checkout, with the developer’s existing authentication.

- [x] Expose a working Claude Code launch choice and launch directly in the adopted checkout. Missing executable, authentication problems and spawn failure show actionable errors without losing context or spawning duplicate attempts.
- [x] On macOS, answer a real native input prompt and approval prompt, exercise native Ctrl-C, complete a turn, return using the documented escape chord and reattach to the same conversation/process.
- [x] Repeated resizing, Unicode/paste and relevant native terminal modes remain usable. Resolve escape-chord conflicts explicitly rather than silently replacing native shortcuts.
- [x] Retain discovery, selection and navigation while Claude Code runs. Use a safe temporary checkout for the native demonstration; do not copy credentials into product state or the evidence record.
- [x] Record exact CLI, macOS and terminal versions, reproducible steps, results and compatibility limitations. Fixture results and real macOS evidence are clearly distinguished; status-signal reliability is established in its separate ticket.
- [x] An unavailable or failed native scenario remains an explicit proof failure requiring resolution; do not advertise established support based only on launch or fixture success. Update working launch/input instructions.


## Comments

2026-11-06: Implemented explicit `c` Claude launch with `l` retained as the safe
fixture choice; direct adopted-checkout PTY launch, tool labels and actionable
spawn/login guidance preserve discovery/adoption. Merged ticket 07 request-result
reconciliation into both launch variants. Native macOS Claude 2.1.318 proof passed
real question/answer, approval/one-time answer, Unicode bracketed paste, repeated
resize, completed turn, Ctrl-C acknowledgement, escape/reattach, detach/reopen and
native exit. Existing native authentication was reused; no policy edits or
credential copying. The CLI auto-updated the symlink to 2.1.319 while the tested
2.1.318 process stayed alive; that newer version is not covered by this proof.
See [versioned evidence and limits](../../../docs/proofs/05-claude-native.md).
The existing nonblocking SessionStart hook failure is recorded without changing
user configuration. Semantic status remains unknown pending ticket 10; this does
not advertise the entire product integration as established support.
