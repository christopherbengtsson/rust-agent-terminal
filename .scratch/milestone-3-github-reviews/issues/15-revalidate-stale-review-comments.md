# 15: Revalidate stale review comments

Status: ready-for-agent
Resolved: no
Blocked by: 13

**What to build:** Detect changed PR comparisons, inspect the latest changes and explicitly reconcile selected comments before they become eligible for submission.

**Acceptance coverage:** R6; R7 examined/validated identity foundation.

## Acceptance criteria

- [ ] Refresh active drafts and mark them stale when the identified head or base/comparison changes, including target-only movement without new author commits. Keep remote freshness and uncertain reads visible.
- [ ] Preserve original examined comparison, findings, anchors and human edits; show the current comparison and changed-since-examination context rather than silently relabeling old content.
- [ ] Require latest-change inspection and explicit revalidation of selected comments against the current comparison. Invalid locations must be relocated or discarded; warning dismissal alone cannot satisfy revalidation.
- [ ] Retain human-validated comparison separately from the agent-examined one. Further revision changes invalidate current submission eligibility again without deleting text.
- [ ] Check draft/text versions when applying validation and relocation so stale actions cannot validate newer content accidentally. Regeneration is not required merely to revalidate.
- [ ] Verify head changes, base-only changes, same-patch revision movement, renames/deletions, stale refresh responses and repeated updates with real temporary Git repositories and the supervisor workflow seam.
- [ ] Demonstrate the complete keyboard inspection/revalidate/relocate flow, durable restart of its state and visible exclusion of unresolved stale comments from submission eligibility.

## Scope and handoff

Anchor semantics come from the comparison proof, not coding working-diff coordinates. This ticket enables local validation; publication still waits for ticket 17.

## Comments
