# 06: Run independent workspaces without mixed controls or history

Status: ready-for-agent
Resolved: no
Blocked by: 03, 05

**What to build:** Launch coding agents in separate workspaces and switch between them while each retains its own input, screen, status, lifecycle and captured history.

- [x] Route input/resize/Stop/reconciliation and observations by stable session identity; rapid selection changes and late replies must never affect a different session.
- [x] Use a session collection and canonical-worktree admission policy. Refuse another active or uncertain coding execution in the same checkout, including aliased/duplicate records; allow distinct workspaces to run concurrently.
- [x] Adapt sole-writer capture to bounded fair per-session batching, handles/accounting and final completions. Interleaving streams must not force a commit on every session switch or confuse durable prefixes.
- [x] Exercise two actual fixture sessions producing interleaved output, focused/detached switching, one stopped while the other continues, storage faults and scoped final flushing without losing accepted byte ownership.
- [x] Preserve per-session supported status/Unknown and distinguish historical attention from current confidence. Generalize ordinary quit/detach/client-loss behavior across the runtime collection.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Scoped implementation complete on `feature/m2-ticket06-runtime`; runtime/test
source `ba17869a6c22701d99ef7516db460bb1b7723875`, including integration
`b74faa31` (03/05/07). See [the evidence record](../../../docs/proofs/m2-06-concurrent-workspaces.md)
for the two actual fixture sessions, targeted controls, alias admission,
per-stream storage faults/final flush, ordinary quit and real dashboard switching
at 160×40 and 60×18. Seven sole-writer, 21 App and 12 UI filtered checks and the
meaningful launch/storage/cleanup representatives pass; final build/fmt/Clippy
pass. The full all-target aggregate remains assigned to the fresh integration
merger after 06/08. Native concurrency and complete milestone qualification remain
ticket 19; no new native support claim is made by this ticket.

Reopened after the combined 06/08 aggregate at `610c723febb759dc76c23bee0f783f96ee679825`:
ownership checks report 61 passed, 12 failed and 2 ignored. Preservation failures
include launch-result replay, retained workspace identity, capture accounting,
history pagination and recovery. Focused concurrency passes do not resolve these
compatibility failures. See [integration evidence](../../../docs/proofs/m2-06-08-integration-gate.md).
Close again only after corrections and the required full compatibility gate pass.
