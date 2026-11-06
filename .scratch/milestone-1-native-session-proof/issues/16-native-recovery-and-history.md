# 16: Prove native recovery independently of history

Status: ready-for-agent
Resolved: yes
Blocked by: 09, 11

**What to build:** Recover a native conversation explicitly when the original tool supports it, while retaining readable history even when native recovery is unavailable.

- [x] For both recorded native tools, capture supported conversation identity where available and explicitly exercise recovery through the tool’s supported native mechanism after exit or interruption.
- [x] Prove native resume continues the original conversation rather than merely launching a new process or showing a transcript. Record unavailable recovery and its reason honestly; a new conversation requires an explicit user choice.
- [x] A supervisor restart or restored interrupted record never automatically starts coding. Preserve uncertain-launch reconciliation and one-active-session-per-worktree restrictions during recovery.
- [x] Show retained readable session history independently of native recovery, including unavailable tool recovery and missing checkout. Explain when checkout loss prevents continuing in place.
- [x] Record real macOS evidence for both tools across dashboard loss, native exit and supervisor failure/reboot-style restoration, using existing authentication without storing credentials.
- [x] Keep this a proof-level recovery path rather than full search, archival or resume management. Document limits and unresolved native failures so they remain visible at the milestone gate.


## Comments

Implemented explicit dashboard `v` recovery through the existing durable launch
and reconciliation gate, using saved original native UUIDs. `c`/`x` remain explicit
new-conversation actions. Actual macOS Claude Code 2.1.318 and Codex CLI 0.167.0
conversations both recalled their original conversation-only markers after native
exit, idle supervisor loss/restart and explicit dashboard resume. Native dashboard
loss preserved the original live processes. Both tools' original readable history
remained accessible when their checkouts were temporarily unavailable; resume was
refused with an explanation. Both isolated supervisors were explicitly stopped,
and proof checkouts remained clean.

See [the exact native proof and limits](../../../docs/proofs/16-native-recovery.md),
including hashes, native/managed identities, captured responses, separate injected
prior-boot fixture restoration, and verification details. Same-boot uncertain
survivor restrictions were preserved; there was no physical reboot or bypass of
that gate. Full historical resume selection/search/archive management remains out
of scope. The final combined all-target run is pending integration; focused final
native/recovery, unchanged safety diagnostic, unit/render, build, formatting and
Clippy checks passed. Earlier aggregate failures and their causal corrections
are recorded rather than described as aggregate passes.

The combined all-target check subsequently passed on `be1b40b`, integrated by
`e2cfef3`: 22 unit tests, 33 ownership/history/recovery scenarios, scripted
interaction/status, and screen integration. Build, formatting, and Clippy also
passed; the earlier pending combined verification is complete. See the ticket 13
record for the exact source hashes and raw results.
