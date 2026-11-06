# 06: Run independent workspaces without mixed controls or history

Status: ready-for-agent
Resolved: no
Blocked by: 03, 05

**What to build:** Launch coding agents in separate workspaces and switch between them while each retains its own input, screen, status, lifecycle and captured history.

- [ ] Route input/resize/Stop/reconciliation and observations by stable session identity; rapid selection changes and late replies must never affect a different session.
- [ ] Use a session collection and canonical-worktree admission policy. Refuse another active or uncertain coding execution in the same checkout, including aliased/duplicate records; allow distinct workspaces to run concurrently.
- [ ] Adapt sole-writer capture to bounded fair per-session batching, handles/accounting and final completions. Interleaving streams must not force a commit on every session switch or confuse durable prefixes.
- [ ] Exercise two actual fixture sessions producing interleaved output, focused/detached switching, one stopped while the other continues, storage faults and scoped final flushing without losing accepted byte ownership.
- [ ] Preserve per-session supported status/Unknown and distinguish historical attention from current confidence. Generalize ordinary quit/detach/client-loss behavior across the runtime collection.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
