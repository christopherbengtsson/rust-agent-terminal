# 08: Create and retain a local review draft

Status: ready-for-agent
Resolved: no
Blocked by: 07

**What to build:** Create a local review draft beside the pinned comparison, add original inline comments and an overall message, and recover exact saved text after navigation and restart without requiring an agent.

**Acceptance coverage:** R4 structured draft foundation.

## Acceptance criteria

- [ ] The user can create and reopen a draft for the selected review cycle and pinned comparison, add location-bearing original comments and enter an overall message.
- [ ] Persist stable draft/comment identities, text versions and original comparison context through transactional migration and the sole writer; do not reconstruct structured records from terminal history.
- [ ] Only the matching attributed save acknowledgement establishes durable success. Save failure retains recoverable user text and last confirmed state; stale/duplicate completions cannot acknowledge another edit.
- [ ] Navigation, reconnect and restart retain exact text and anchor context. The draft remains distinct from preparation attempts and published reviews.
- [ ] No draft operation launches a reviewer, chooses an outcome, selects comments implicitly or writes to GitHub. Original-comment publication selection is completed in ticket 13.
- [ ] Verify the supervisor workflow with real temporary storage, migration/restart, write failures and reordered or lost acknowledgements, independently of transcript/index availability.

## Scope and handoff

Introduce only the records needed for this complete manual draft slice, leaving later schema additions with their feature tickets. Preserve coding capture and sole-writer ownership.

## Comments
