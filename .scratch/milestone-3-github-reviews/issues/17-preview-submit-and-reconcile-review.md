# 17: Preview, submit and reconcile a review

Status: ready-for-agent
Resolved: no
Blocked by: 05, 15

**What to build:** Choose Comment, Approve or Request changes, preview exactly what will be sent, explicitly submit, and recover uncertain GitHub outcomes without duplicates.

**Acceptance coverage:** R5, R6 publication checks, R11.

## Acceptance criteria

- [ ] Only the user selects the outcome. The final preview identifies PR, exact comparison, outcome, overall message and exact selected inline comments; undecided/discarded suggestions and unselected originals are absent.
- [ ] Publication requires an explicit submission after that preview. Freeze the content identity so intervening edits cannot silently alter the payload; no preparation, editor, navigation or settings path authorizes a write.
- [ ] Refresh eligibility and comparison before sending. A revision change requires current inspection/revalidation and a new preview, including target-only changes. Apply the proven remote-race handling and expose its limits.
- [ ] Commit an attributed publication intent before external effects and retain enough remote evidence to reconcile the same intent. Only matching evidence establishes confirmed submission.
- [ ] Definite failure preserves the draft. A response lost after possible success remains uncertain; reconcile remote review/comment identities before permitting any duplicate-capable retry, including after dashboard/supervisor restart.
- [ ] Exercise existing pending reviews, partial effects, delayed reads, duplicate actions and unresolved reconciliation. Never label an uncertain result confirmed failure or success merely to unblock retry.
- [ ] Verify all three outcomes and exact selected-content behavior with controlled GitHub faults. Real-service qualification requires the user to preview the exact revision/outcome/comments and explicitly submit each actual review; the implementer cannot approve its own write.
- [ ] At the supervisor seam, prove that remote-success/response-loss followed by reconciliation yields one final review and one copy of each selected comment. Publication and its recovery safeguards must land together.

## Scope and handoff

Use ticket 05 evidence; do not defer uncertain-write safety to the lifecycle or final-qualification ticket. Manual drafts may exercise publication independently of native preparation or external-editor completion.

## Comments
