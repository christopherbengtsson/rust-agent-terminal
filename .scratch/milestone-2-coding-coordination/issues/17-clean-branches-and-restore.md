# 17: Clean local branches while preserving archive recovery

Status: ready-for-agent
Resolved: yes
Blocked by: 16

**What to build:** Archiving cleans app-created local branch names by default, and returning to archived work safely restores a usable branch.

- [x] Expose Delete local branch on archive, default on for product-created branches and off for adopted existing branches unless explicitly selected. Revalidate branch ownership after external rename/switch; never delete an unrelated branch.
- [x] Only delete after saved-code recovery ref/metadata are confirmed. Retain folder/ignored files, safely detach at saved commit where necessary, and never delete remote branches or a branch in use elsewhere.
- [x] Recheck expected ref/head and worktree identity immediately before mutation; retain external new commits and report cleanup failure separately from confirmed save/archive state.
- [x] Restore/recreate a branch from the retained recovery point, resolving reused old names without overwriting refs. Missing folders stay honestly unavailable; native resume in place is not falsely promised.
- [x] Exercise GC, crash/lost-reply/retry windows, branch-name reuse, adopted/main/default branch protection and local-only commits in real repositories. Preserve future PR/review identity independently of branch names.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Implemented with immutable provenance/policy, durable branch-operation receipts,
ordinary guarded branch deletion and create-only restoration markers. Cleanup
failure remains separate from confirmed archive saving. Automatic deletion is
qualified for Git 2.50/files refs with compatible hook policy; unsupported or
uncertain cases retain the branch. Worktree-use checks are immediate snapshots,
not a universal lock against external Git. Exact-revision Return never starts
coding and never overwrites reused names.

Validation and limitations: [ticket 17 proof](../../../docs/proofs/m2-17-branches.md).
140 binary tests passed (3 explicit corpus/probe ignores), all 11 Controller/
outer-PTY archive scenarios passed, and final expected-tip guard/16 cleanup checks
passed after query hardening. Locked build, format, all-target Clippy and diff
checks passed. Six normal/small keyboard captures are linked in the proof.
Native/combined/reboot qualification remains ticket 19; no physical reboot or
universal external-Git atomicity is claimed.
