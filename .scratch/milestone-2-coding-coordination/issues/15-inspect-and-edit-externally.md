# 15: Inspect diffs and edit source with reliable editor return

Status: ready-for-agent
Resolved: yes
Blocked by: 14

**What to build:** Open either comparison in Vim or a configured editor, open real workspace files for edits, and return to the same dashboard context.

- [x] Support executable-plus-arguments configuration without shell interpolation; materialize labelled comparison artifacts independently of checkout-local temporary filenames.
- [x] Open the workspace or selected existing source file for actual editing, positioning where supported. Deleted/missing files remain inspectable without invented editable paths.
- [x] Release/reacquire terminal ownership for terminal editors and restore selected project/workspace/session, focus, diff/history context and usable geometry on success, failure and resize.
- [x] Agents continue in the supervisor; diff artifacts remain stable while open and reopening obtains fresh data. Live source edits are explicitly distinct from inspection snapshots.
- [x] Record actual macOS Vim and one alternative-editor checks for Working/Whole workspace, source saving, nonzero exit, missing executable and return. Preserve reference composition and defer inline source editing.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Implemented in `ticket15-external-editor` from integration `ea7deea`.
See [the versioned editor proof](../../../docs/proofs/m2-15-external-editor.md)
for actual Vim 9.1/Pico 5.09 Working/Whole inspection, live source saving,
Ctrl-C/foreground ownership, resize, nonzero/missing-executable return and retained
supervisor activity, plus the initial cursor-report failure/fix. Execution-owned
full-comparison content and source validation stay separate from dashboard-local
artifact materialization and terminal/editor-worker ownership. The later filtered
App regression retains selected history passage/raw/scroll and diff context across
an intervening observation and refuses stale native focus. Native captures retain
their exact pre-context-hardening source boundary; ticket 19 owns complete
final-source combined native/reboot/resource qualification.
