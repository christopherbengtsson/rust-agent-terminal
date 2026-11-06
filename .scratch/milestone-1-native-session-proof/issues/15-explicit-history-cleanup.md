# 15: Offer explicit history cleanup

Status: ready-for-agent
Blocked by: 14
Resolved: yes

**What to build:** Respond to low storage by inspecting retained history and explicitly choosing what to delete, while keeping working checkouts safe.

- [x] Warn before storage becomes critically low using a documented, testable threshold and expose cleanup showing task, session age and stored size, suggesting oldest stopped sessions first.
- [x] Require explicit selection and deletion; age, archival, session exit and storage pressure never authorize automatic cleanup. Scope this slice to retained stopped-session history rather than broad archival management.
- [x] Explain the selected deletion’s effect before applying it. Deleting history does not remove or modify checkout files, task working changes or another session’s transcript.
- [x] After successful deletion, update retained-history navigation and storage accounting consistently. Test persistence restart so partially failed cleanup cannot masquerade as successful deletion.
- [x] If deletion fails or frees insufficient space, preserve a visible failure and storage fault with retry/stop available; do not claim capture recovered until durable writes succeed.
- [x] Integration and state/render checks exercise warning, ordering, explicit deletion, cancellation, failed cleanup and continued low storage. Document the explicit-retention policy and recovery actions.


## Completion evidence

Implemented and verified in `7f8a41d`, including integration `a5b406f` and reusable
measurement telemetry checkpoint `2c9c54c`. See
[the proof record](../../../docs/proofs/15-explicit-history-cleanup.md) for explicit
retention controls, the 1 GiB warning threshold, bounded list/unlink work, durable
intent/directory-sync/completed-marker ordering, restart/failed cleanup evidence,
physical size/unknown legacy-age policy, capture safety and native segment isolation.

Locked build, formatting and all-target warnings-denied Clippy passed. Final
all-target regression excluding only the unchanged long sustained resource case
passed 26 unit tests (the ignored restoration probe is exercised by its driver),
46 ownership scenarios (one ignored measurement, one filtered resource case),
scripted interaction and screen integration. No new real native CLI experiments,
physical disk-full/power-loss proof, graphical terminal presentation measurement,
or final union resource gate is claimed; ticket 17/18 retain those responsibilities.
Legacy stopped records without an accepted EOF/final-flush seal remain unavailable
for deletion, with explicitly unknown age; process Stopped alone never grants
cleanup eligibility. Pending capture/low capacity/fault context survives cleanup.
