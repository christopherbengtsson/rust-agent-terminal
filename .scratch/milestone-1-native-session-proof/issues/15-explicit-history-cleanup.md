# 15: Offer explicit history cleanup

Status: ready-for-agent
Blocked by: 14

**What to build:** Respond to low storage by inspecting retained history and explicitly choosing what to delete, while keeping working checkouts safe.

- [ ] Warn before storage becomes critically low using a documented, testable threshold and expose cleanup showing task, session age and stored size, suggesting oldest stopped sessions first.
- [ ] Require explicit selection and deletion; age, archival, session exit and storage pressure never authorize automatic cleanup. Scope this slice to retained stopped-session history rather than broad archival management.
- [ ] Explain the selected deletion’s effect before applying it. Deleting history does not remove or modify checkout files, task working changes or another session’s transcript.
- [ ] After successful deletion, update retained-history navigation and storage accounting consistently. Test persistence restart so partially failed cleanup cannot masquerade as successful deletion.
- [ ] If deletion fails or frees insufficient space, preserve a visible failure and storage fault with retry/stop available; do not claim capture recovered until durable writes succeed.
- [ ] Integration and state/render checks exercise warning, ordering, explicit deletion, cancellation, failed cleanup and continued low storage. Document the explicit-retention policy and recovery actions.
