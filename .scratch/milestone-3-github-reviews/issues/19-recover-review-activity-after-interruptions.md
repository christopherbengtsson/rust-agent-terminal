# 19: Recover review activity across app interruptions

Status: ready-for-agent
Resolved: no
Blocked by: 12, 14, 16, 18

**What to build:** Make the full review workflow obey detach, quit, dashboard loss and restart semantics while preserving drafts, recovery edits, queue state and uncertain-publication evidence.

**Acceptance coverage:** R2 lifecycle; R4 retained edits; R7 preserved generations; R10; R11 restart recovery.

## Acceptance criteria

- [ ] Detach or dashboard loss leaves supervisor-owned discovery/preparation running while the Mac is awake and logged in; reconnect targets the same retained operations rather than starting duplicates.
- [ ] Ordinary quit stops owned local review activity and coding activity according to existing behavior while retaining queue, drafts, edits and publication uncertainty. Stopping preparation never silently discards human work.
- [ ] On reopening after quit or reboot, resume discovery and eligible automatic preparation under the saved informed setting. Interrupted attempts remain visible, with eligibility/comparison refreshed before fresh attempts and retry budgets preserved.
- [ ] Explicitly cancelled preparation stays cancelled until the user restarts it. Paused/disabled settings remain effective; no stale save or worker completion can rearm cancelled work.
- [ ] Restore recoverable external-editor text and its original target/version without silently applying it, overwriting newer text or confusing staging with a committed draft save.
- [ ] Restore pending/uncertain publication for read-side reconciliation only. No ordinary startup, reconnect, retry timer or preparation recovery repeats a GitHub write or automatically restarts coding.
- [ ] Exercise abrupt dashboard/supervisor failure around preparation, regeneration, editor readback, durable saves and publication acknowledgement with deterministic fixtures and macOS process/PTY checks.
- [ ] Coordinate real sleep/reboot actions with the user where selected for native recovery evidence; record simulated versus physical evidence separately and keep existing coding recovery guarantees intact.

## Scope and handoff

This joins already-safe feature lifecycles; it is not permission to defer per-feature cancellation, attribution or uncertain-publication handling until now.

## Comments
