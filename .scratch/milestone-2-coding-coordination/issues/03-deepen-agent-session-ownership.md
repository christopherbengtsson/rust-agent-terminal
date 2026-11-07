# 03: Deepen one Agent session runtime

Status: ready-for-agent
Resolved: yes
Blocked by: 01, 02

**What to build:** The existing one-session workflow keeps working through one concrete session owner that handles provisional launch, native controls, stop and final history completion.

- [x] Move session progression incrementally behind a concrete module with private launch, execution and history responsibilities; the supervisor routes and observes instead of reconstructing phase flags.
- [x] Keep screen/input/resize/Stop usable while native execution exists but result durability is provisional. Keep process stop separate from settled/sealed history.
- [x] Keep workspace admission, transport/controller ownership, Git state and SQLite implementation outside the session owner. Introduce no generic plugin/callback framework or unbounded per-session execution threads.
- [x] Preserve all existing public behaviors across lost replies, explicit reconciliation, cancelled preparation, receipt gaps and pending/full-storage capture. Restore never starts or resumes coding.
- [x] Use the established dashboard/supervisor/fixture seam and focused state checks; retain real process evidence and all meaningful native proof obligations.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Implemented in `125bf5a` with private concrete Launch, Execution and History
ownership behind the polled `session::agent_session::AgentSession` interface.
Transport, canonical Git admission and Git workers remain supervisor-owned;
SQLite and the sole storage worker remain persistence-owned. Observation is
projected by value; no private phase fields are reconstructed by the caller.

Locked build, fmt, all-target warnings-denied Clippy and supporting bin checks
(32 passed, one probe ignored directly) passed. All 13 serial focused ownership
checks passed, including strengthened provisional result input/resize/screen/Stop,
operation attribution, partial spawn/failed cleanup, cancellation, capture EOF,
storage retry and separate-process-group Stop/quit/native-exit cleanup. Current
integration `50c0bae` was merged before reporting. See
[the scoped proof](../../../docs/proofs/m2-03-agent-session-owner.md).
The combined 03/05 aggregate gate remains integration-owned; prior capture-byte
fixture failures remain separately recorded rather than asserted fixed here.
No new native CLI/reboot/resource qualification is claimed.
