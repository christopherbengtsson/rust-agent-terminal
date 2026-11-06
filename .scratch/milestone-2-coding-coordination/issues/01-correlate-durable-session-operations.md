# 01: Correlate durable session operations

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** Starting, stopping and retrying the existing single session keep the same behavior, while only the exact matching durable completion can settle its launch outcome.

- [ ] Attach stable session/request, operation identity and purpose to storage commands and both successful and failed completions; unrelated saves cannot authorize launch or acknowledge its result.
- [ ] Preserve durable preparation, authorization and result ordering, same-request replay, cancellation during stalled storage and conservative restored uncertainty.
- [ ] Verify stale, duplicate and mismatched completions do not advance another obligation or overwrite newer execution facts, using real isolated storage and the existing process seam.
- [ ] Keep current wire behavior working during the change; detect incompatible live owners before migration or incompatible messaging. Document the compatibility strategy.
- [ ] Retain existing single-session lost-acknowledgement, crash-window and provisional-receipt acceptance behavior; report actual checks.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
