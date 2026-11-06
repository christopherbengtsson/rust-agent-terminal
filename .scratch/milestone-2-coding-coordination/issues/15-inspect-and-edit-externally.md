# 15: Inspect diffs and edit source with reliable editor return

Status: ready-for-agent
Resolved: no
Blocked by: 14

**What to build:** Open either comparison in Vim or a configured editor, open real workspace files for edits, and return to the same dashboard context.

- [ ] Support executable-plus-arguments configuration without shell interpolation; materialize labelled comparison artifacts independently of checkout-local temporary filenames.
- [ ] Open the workspace or selected existing source file for actual editing, positioning where supported. Deleted/missing files remain inspectable without invented editable paths.
- [ ] Release/reacquire terminal ownership for terminal editors and restore selected project/workspace/session, focus, diff/history context and usable geometry on success, failure and resize.
- [ ] Agents continue in the supervisor; diff artifacts remain stable while open and reopening obtains fresh data. Live source edits are explicitly distinct from inspection snapshots.
- [ ] Record actual macOS Vim and one alternative-editor checks for Working/Whole workspace, source saving, nonzero exit, missing executable and return. Preserve reference composition and defer inline source editing.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
