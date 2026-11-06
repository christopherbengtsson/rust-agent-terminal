# 14: Recover from slow or full storage

Status: ready-for-agent
Blocked by: 13

**What to build:** Recognize history capture trouble and retain control of recovery or stop when storage is slow, failing or full.

- [ ] Inject slow writes, write errors and full-storage behavior through the existing integration seam, without filling the developer’s real disk. Display the failure and capture/backpressure condition promptly.
- [ ] Finite buffers apply explicit backpressure instead of silently discarding accepted transcript data. Saved metadata is not acknowledged before durable success; failed writes are never reported as saved.
- [ ] Keep dashboard navigation, recovery commands and explicit stop responsive while storage stalls. Reserve or otherwise preserve the control path when output queues saturate.
- [ ] Demonstrate successful retry and resumed capture with retained accepted data; persistent failure stays visible with retry/stop available. Clean-shutdown flush failures remain actionable rather than a false success.
- [ ] Compare observable emitted, accepted and durable data across faults and abrupt failure. Explain any unflushed crash loss; do not claim that pausing PTY reads suspends all agent work or descendants.
- [ ] Record memory/queue bounds, failure timings, producer blocking and preliminary fault responsiveness. No automatic history deletion or unsupported safe-suspension claim is permitted.
