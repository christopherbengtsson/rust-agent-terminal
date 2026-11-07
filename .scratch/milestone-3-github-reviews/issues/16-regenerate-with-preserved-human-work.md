# 16: Regenerate without losing human work

Status: ready-for-agent
Resolved: no
Blocked by: 10, 15

**What to build:** Explicitly regenerate a draft for current code while retaining earlier examined versions, human edits and choices as historical context.

**Acceptance coverage:** R7; R6 optional regeneration.

## Acceptance criteria

- [ ] Regeneration requires an explicit action even with automatic preparation enabled. It resolves the current eligible comparison and guidance without silently changing an existing draft.
- [ ] Retain earlier draft generations, examined/validated comparisons, exact human text and choices so they remain accessible beside their original context.
- [ ] New suggestions start undecided; previous kept/submitted choices do not transfer as publication permission. Clearly distinguish a complete replacement from failed or incomplete regeneration.
- [ ] Changed fallback settings or target guidance apply to the new preparation and its recorded provenance; earlier generations keep their original guidance.
- [ ] Cancellation, provider failure or storage failure preserves the previous usable draft and recoverable new results. Returning external edits cannot overwrite the new generation silently.
- [ ] Verify stale regeneration, no-findings replacement, repeated attempts and storage/interruption races through the supervisor workflow with controlled reviewers; exercise a real qualified reviewer under cost controls.

## Scope and handoff

Reuse serial admission and attempt identity; do not add a second independent reviewer runner. Existing drafts must remain usable if regeneration fails.

## Comments
