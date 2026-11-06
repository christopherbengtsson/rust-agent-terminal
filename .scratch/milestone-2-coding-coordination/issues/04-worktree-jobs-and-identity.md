# 04: Resolve worktrees through bounded execution-owned jobs

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** Discovery, refresh and adoption resolve actual checkouts without blocking navigation or shared session controls.

- [ ] Concentrate existing discovery and adoption Git/path interpretation in execution-owned workers. Preserve original Unix path bytes, NUL-safe parsing and canonical worktree identity.
- [ ] A slow or failed Git process is bounded/cancellable, leaves prior successful discovery visible, and cannot block Stop, session polling or dashboard exit.
- [ ] Route results by request/project/checkout identity so late refresh/adoption replies cannot retarget the current selection. Keep discovery read-only and adoption explicit.
- [ ] Verify actual temporary linked, aliased, detached, bare, locked, missing and prunable worktrees, including non-UTF-8 paths and saturated dispatch.
- [ ] Establish a concrete per-repository mutation execution discipline for subsequent creation/archive work without holding exclusion for whole agent sessions or creating a generic job engine.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
