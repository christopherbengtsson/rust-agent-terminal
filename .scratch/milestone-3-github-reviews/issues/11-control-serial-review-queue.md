# 11: Control the serial preparation queue

Status: ready-for-agent
Resolved: no
Blocked by: 10

**What to build:** Prepare eligible requests serially with visible queue control, cancellation, eligibility changes and bounded preparation retries, without serializing coding sessions.

**Acceptance coverage:** R2, R9, R10.

## Acceptance criteria

- [ ] Expose queued, preparing, ready, stale, failed, incomplete and submitted distinctions appropriate to implemented behavior, with one active reviewer across PRs and independent coding admission.
- [ ] Pause queue prevents new preparations while the active attempt may finish. Cancel preparation stops the active attempt; cancellation stays cancelled until explicit restart.
- [ ] Use manual enqueue/start actions until informed setup enables automation. Preserve queue/request/attempt identities and choices across reconnect without hidden agent starts.
- [ ] Refresh eligibility and comparison before an attempt; remove closed, merged or withdrawn requests from eligibility without deleting saved drafts.
- [ ] Temporary preparation failures receive at most two automatic retries with increasing delays, visible count and next retry time. Persistent/authentication/guidance/restriction failures require user action, and manual retry remains available after exhaustion.
- [ ] Retries retain prior partial results and human edits. Incomplete inspection caused by coverage/resource limits requires explicit action rather than this transient retry policy.
- [ ] Verify pause/cancel races, stale worker results, eligibility withdrawal, failure classes and deterministic backoff through the supervisor seam while coding fixtures stream. No queue action can publish or retry publication.

## Scope and handoff

Ticket 12 activates informed automatic enrollment and settings; ticket 19 completes app-lifecycle recovery. Persist only concrete queue state, not a general-purpose job engine.

## Comments
