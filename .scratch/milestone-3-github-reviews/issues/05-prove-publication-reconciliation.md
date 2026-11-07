# 05: Establish publication reconciliation semantics

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** Prove an implementable strategy for publishing an exact review and reconciling uncertain remote outcomes without duplicate reviews or comments.

**Acceptance coverage:** Engineering prerequisite for R5 and R11.

## Acceptance criteria

- [ ] Establish supported GitHub mechanisms for Comment, Approve and Request changes, overall text and selected inline comments at identified revisions, including eligibility restrictions and existing pending reviews.
- [ ] Define the evidence that correlates one local publication intent with remote review/comment identities, including a remote write that succeeds before its response is lost.
- [ ] Exercise controlled remote-success/response-loss, definite rejection, partial effects, reordered or delayed responses and reads that cannot yet resolve uncertainty. Demonstrate that an unresolved prior write blocks duplicate-capable retries.
- [ ] Identify publication race limits, including revision/eligibility changes after preflight and before remote acceptance. Document what GitHub can enforce and how original revision identity remains explainable.
- [ ] Use primary GitHub documentation and authenticated read evidence alongside executable fault fixtures. Clearly separate fixture conclusions from real-service proof.
- [ ] Any real review publication requires the user to preview the exact PR, revision, selected comments and outcome and explicitly submit. Research, agent decisions and this ticket do not authorize test review publication.
- [ ] Retain a concrete strategy and reproducible evidence for ticket 17. If duplicate-safe reconciliation cannot be established, return for a decision; never treat uncertainty as confirmed failure merely to enable retry.

## Scope and handoff

No discovery or UI implementation is needed for this proof. Publication and reconciliation ship together in ticket 17; do not introduce a happy-path write now and defer its recovery safeguards.

## Comments
