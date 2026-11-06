# 16: Prove native recovery independently of history

Status: ready-for-agent
Blocked by: 09, 11

**What to build:** Recover a native conversation explicitly when the original tool supports it, while retaining readable history even when native recovery is unavailable.

- [ ] For both recorded native tools, capture supported conversation identity where available and explicitly exercise recovery through the tool’s supported native mechanism after exit or interruption.
- [ ] Prove native resume continues the original conversation rather than merely launching a new process or showing a transcript. Record unavailable recovery and its reason honestly; a new conversation requires an explicit user choice.
- [ ] A supervisor restart or restored interrupted record never automatically starts coding. Preserve uncertain-launch reconciliation and one-active-session-per-worktree restrictions during recovery.
- [ ] Show retained readable session history independently of native recovery, including unavailable tool recovery and missing checkout. Explain when checkout loss prevents continuing in place.
- [ ] Record real macOS evidence for both tools across dashboard loss, native exit and supervisor failure/reboot-style restoration, using existing authentication without storing credentials.
- [ ] Keep this a proof-level recovery path rather than full search, archival or resume management. Document limits and unresolved native failures so they remain visible at the milestone gate.
