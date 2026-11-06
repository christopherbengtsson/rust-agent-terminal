# 13: Inspect all working changes in the dashboard

Status: ready-for-agent
Resolved: no
Blocked by: 05

**What to build:** Open a workspace diff and inspect staged, unstaged and new files with keyboard file navigation and visible comparison freshness.

- [ ] Working is the default every time inspection opens. Distinguish staged/unstaged/new-file state, including overlapping edits, new files, renames, deletions, binaries and conflicts.
- [ ] Generate comparisons in execution-owned bounded workers without changing the user index, running arbitrary external diff commands or blocking input/Stop.
- [ ] Label workspace/comparison identity and freshness; refreshed results preserve selected file/context where possible and stale responses cannot replace another workspace view.
- [ ] Use safe printable display and bounded file/content presentation. Explain unsupported, submodule, missing-file and oversized cases instead of silently omitting changed work.
- [ ] Verify actual Git repositories and dashboard rendering/keyboard behavior while an agent changes files, including normal and small terminal sizes.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
