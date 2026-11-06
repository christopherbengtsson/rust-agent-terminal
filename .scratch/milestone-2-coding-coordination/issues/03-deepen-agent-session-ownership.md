# 03: Deepen one Agent session runtime

Status: ready-for-agent
Resolved: no
Blocked by: 01, 02

**What to build:** The existing one-session workflow keeps working through one concrete session owner that handles provisional launch, native controls, stop and final history completion.

- [ ] Move session progression incrementally behind a concrete module with private launch, execution and history responsibilities; the supervisor routes and observes instead of reconstructing phase flags.
- [ ] Keep screen/input/resize/Stop usable while native execution exists but result durability is provisional. Keep process stop separate from settled/sealed history.
- [ ] Keep workspace admission, transport/controller ownership, Git state and SQLite implementation outside the session owner. Introduce no generic plugin/callback framework or unbounded per-session execution threads.
- [ ] Preserve all existing public behaviors across lost replies, explicit reconciliation, cancelled preparation, receipt gaps and pending/full-storage capture. Restore never starts or resumes coding.
- [ ] Use the established dashboard/supervisor/fixture seam and focused state checks; retain real process evidence and all meaningful native proof obligations.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
