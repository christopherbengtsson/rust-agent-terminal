# 16: Stop, save and archive a workspace

Status: ready-for-agent
Resolved: no
Blocked by: 10

**What to build:** Stop and archive saves unfinished code locally, reports exactly what was saved and hides the workspace without deleting its folder or chats.

- [ ] Cancel pending launch/setup intent and establish confirmed owned-execution cleanup and settled capture before checkpoint; block new coding admission while mutation is pending.
- [ ] Save tracked and new non-ignored changes as a clearly named Archive checkpoint commit; avoid empty commits on clean work. Preserve ignored files and explain exclusions.
- [ ] Handle conflicts, submodules/nested state, Git identity/hooks errors and concurrent external edits without false save success, bypassing hooks or silently changing unrelated work/index state.
- [ ] Pin saved HEAD with a workspace-specific recovery ref and durable workspace metadata before reporting recovery protection. Lost replies/crashes reconcile prior save instead of duplicating commits.
- [ ] Confirm local checkpoint ID/not-pushed coverage and archive visibility separately. Failed saving leaves unarchived; unarchive restores visibility without coding restart. Verify retained code after Git GC and transcript deletion.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
