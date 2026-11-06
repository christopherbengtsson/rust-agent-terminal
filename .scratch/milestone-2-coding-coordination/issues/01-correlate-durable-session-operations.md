# 01: Correlate durable session operations

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** Starting, stopping and retrying the existing single session keep the same behavior, while only the exact matching durable completion can settle its launch outcome.

- [x] Attach stable session/request, operation identity and purpose to storage commands and both successful and failed completions; unrelated saves cannot authorize launch or acknowledge its result.
- [x] Preserve durable preparation, authorization and result ordering, same-request replay, cancellation during stalled storage and conservative restored uncertainty.
- [x] Verify stale, duplicate and mismatched completions do not advance another obligation or overwrite newer execution facts, using real isolated storage and the existing process seam.
- [x] Keep current wire behavior working during the change; detect incompatible live owners before migration or incompatible messaging. Document the compatibility strategy.
- [ ] Retain existing single-session lost-acknowledgement, crash-window and provisional-receipt acceptance behavior; report actual checks.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Implementation prepared on `ticket/01-durable-session-operations` in the isolated
`/tmp/rat-ticket-01` worktree, based on integration commit `55f1e17` (ancestry
confirmed before implementation). Exact writer obligations now correlate success
and failure through preparation, authorization, result, metadata, final flush and
seal. Tests use the agreed real-process/storage seam; altered/duplicate/prior-
session receipts and failed-result retry are covered. The public wire and SQLite
schema are unchanged; the existing live-owner lock remains the migration barrier.

See [implementation and check evidence](../../../docs/proofs/m2-01-durable-session-operations.md)
for the red/green observations, focused passing checks, initial aggregate failures
and corrections, compatibility strategy and unverified native behavior. `Resolved:
no` remains intentional: the integration owner will run the serialized whole-union
all-target gate, including the unchanged sustained resource assertion, before
claiming completion. No acceptance assertion was weakened.
