# 14: Recover from slow or full storage

Status: ready-for-agent
Resolved: yes
Blocked by: 13

**What to build:** Recognize history capture trouble and retain control of recovery or stop when storage is slow, failing or full.

- [x] Inject slow writes, write errors and full-storage behavior through the existing integration seam, without filling the developer’s real disk. Display the failure and capture/backpressure condition promptly.
- [x] Finite buffers apply explicit backpressure instead of silently discarding accepted transcript data. Saved metadata is not acknowledged before durable success; failed writes are never reported as saved.
- [x] Keep dashboard navigation, recovery commands and explicit stop responsive while storage stalls. Reserve or otherwise preserve the control path when output queues saturate.
- [x] Demonstrate successful retry and resumed capture with retained accepted data; persistent failure stays visible with retry/stop available. Clean-shutdown flush failures remain actionable rather than a false success.
- [x] Compare observable emitted, accepted and durable data across faults and abrupt failure. Explain any unflushed crash loss; do not claim that pausing PTY reads suspends all agent work or descendants.
- [x] Record memory/queue bounds, failure timings, producer blocking and preliminary fault responsiveness. No automatic history deletion or unsupported safe-suspension claim is permitted.


## Evidence

Implemented and verified through the approved real dashboard/supervisor/scripted
PTY and App/render seams. [Fault proof](../../../docs/proofs/14-storage-faults.md)
records injected 250ms writes/1500ms syncs, true prefix-write ENOSPC before chunk
rotation, persistent failure, retry, stalled launch metadata controls, Stop,
shutdown repair, exact accepted-byte continuity and abrupt durable-prefix recovery.
Raw source-labelled queue/RSS/emission/control observations are linked there.

All 39 ownership scenarios and the scripted/screen integrations passed together
on the combined source before the final warning-only placement change; its
render regression and build/fmt/clippy pass separately. Final loss disclosure
remains visible during an active storage fault. A further exact-source aggregate
repeat is in progress and will be appended to the proof. No native version,
physical presentation, actual full-volume or fault usable-attach latency claim
is inferred. Ticket 17 retains those measurement gates. All accepted bytes in
the recorded fault run became durable; 1024 completed-emitted but uncaptured
kernel/PTY bytes did not survive explicit Stop, disclosed separately from accepted
history. No automatic history deletion or safe-suspension claim was introduced.


Follow-up: the extra exact-source repeat exposed an intermittent Authorize-to-exec
receipt pending-state gap. A deterministic held-receipt regression reproduced
it, then passed after the pending predicate included receipt/outcome phases and
Shutdown recognized the current owned stopping worker. The proof records the
38/39 failed repeat and the correction; no null result is accepted as durable.
