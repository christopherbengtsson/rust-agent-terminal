# 18: Remove eligible workspace folders without losing saved work

Status: ready-for-agent
Resolved: yes
Blocked by: 17

**What to build:** Preview and explicitly remove an eligible linked workspace folder while preserving code recovery, history and unrelated repository state.

- [x] Keep removal separate from archive/history deletion. Exclude main checkout, locked/prunable/unsafe identity, active or uncertain execution, pending capture and incomplete archive mutations.
- [x] Block unsaved tracked, untracked and ignored files; list actionable blockers. Treat uncertain upstream/push checks as uncertainty, not proof of remote backup.
- [x] Before removal, verify or create a durable recovery ref for the exact current HEAD, including never-archived and detached worktrees; retain prior archive snapshots. Warn/confirm local-only code, then retain all recovery refs, saved commits, history and any non-deleted branch.
- [x] Revalidate identity/blockers immediately before Git removal; honor Git refusal if external changes invalidate the preview. Recover partial outcomes without deleting arbitrary recorded paths.
- [x] Verify local-only commits, missing upstream/remote, ignored files, path replacement, locks and lost acknowledgements. Include never-archived detached HEAD → folder removal → Git GC → saved-code recovery; history stays usable without checkout.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.

Implemented on source `f8fd7cc` with a distinct preview/confirmation/durable receipt
flow and ordinary Git removal. Exact current HEAD gets a retained recovery ref;
unsaved ignored files and uncertain push/execution state block removal. Restart
and explicit reconciliation never replay deletion. Evidence, seven Controller/PTY
scenarios, 12 real-Git cases, source checks and remaining ticket 19 limits are in
[the removal proof](../../../docs/proofs/m2-18-removal.md). Full native, combined
resource and actual reboot qualification remains ticket 19.
