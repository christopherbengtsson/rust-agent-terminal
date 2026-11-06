# 17: Clean local branches while preserving archive recovery

Status: ready-for-agent
Resolved: no
Blocked by: 16

**What to build:** Archiving cleans app-created local branch names by default, and returning to archived work safely restores a usable branch.

- [ ] Expose Delete local branch on archive, default on for product-created branches and off for adopted existing branches unless explicitly selected. Revalidate branch ownership after external rename/switch; never delete an unrelated branch.
- [ ] Only delete after saved-code recovery ref/metadata are confirmed. Retain folder/ignored files, safely detach at saved commit where necessary, and never delete remote branches or a branch in use elsewhere.
- [ ] Recheck expected ref/head and worktree identity immediately before mutation; retain external new commits and report cleanup failure separately from confirmed save/archive state.
- [ ] Restore/recreate a branch from the retained recovery point, resolving reused old names without overwriting refs. Missing folders stay honestly unavailable; native resume in place is not falsely promised.
- [ ] Exercise GC, crash/lost-reply/retry windows, branch-name reuse, adopted/main/default branch protection and local-only commits in real repositories. Preserve future PR/review identity independently of branch names.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
