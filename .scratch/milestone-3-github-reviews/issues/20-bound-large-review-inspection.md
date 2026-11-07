# 20: Handle large reviews with bounded resources

Status: ready-for-agent
Resolved: no
Blocked by: 11

**What to build:** Inspect large PRs in manageable pieces and show truthful coverage when provider or operational limits prevent completion, while coding remains responsive.

**Acceptance coverage:** R2 concurrent work; R3 complete/incomplete review meaning; R10 resource failures.

## Acceptance criteria

- [ ] Inspect changes in manageable pieces where feasible, retaining pinned comparison and guidance identity across all pieces and reporting what was actually covered.
- [ ] No arbitrary product cap on PR size, file count or review duration is introduced. Establish bounded memory/queues/process output and necessary operational resource handling; return proposed user-visible restrictions for an explicit decision.
- [ ] Provider context exhaustion, account usage limits or operational limits that prevent inspection completion preserve findings and show an incomplete result with coverage and reason, never a ready no-findings draft.
- [ ] Incomplete coverage requires explicit action to try again. Do not silently loop pieces indefinitely, auto-retry incomplete inspection under transient retry rules, or lose human edits on recovery.
- [ ] Coding navigation, attach, Stop and durable capture/search remain available under concurrent restricted review and slow/full storage. Keep serial review admission independent of coding concurrency and writer scheduling.
- [ ] Verify large file inventories, oversized patches, binary/unavailable context, bounded output and interrupted pieces with controlled reviewers and real temporary Git repositories; measure resource use and responsiveness.
- [ ] Record supported workloads, source/tool versions and measured limits. Select and run affected six-stream, capture/history and full-storage supplements as required; distinguish unverified scope from passing evidence.

## Scope and handoff

Build on the working preparation queue rather than creating a separate batch framework. Full combined real-native release qualification remains Milestone 5; this ticket must still demonstrate M3 concurrency behavior.

## Comments
