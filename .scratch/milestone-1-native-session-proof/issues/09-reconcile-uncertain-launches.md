# 09: Reconcile launches after supervisor failure

Status: ready-for-agent
Blocked by: 07, 08

**What to build:** After supervisor failure or reboot, understand interrupted work and prevent replacement coding from competing with an uncertain surviving child.

- [ ] Persist launch intent and outcomes with stable request/session identities so failure around process creation restores interrupted or uncertain records instead of silently relaunching.
- [ ] Reconcile whether a child survived before releasing worktree exclusivity. Unresolved identity or survival blocks a replacement launch with a visible explanation and explicit recovery path; a reused PID alone is insufficient evidence.
- [ ] Reconnect recovers known outcomes for the same request across supervisor restart. Do not claim exactly-once execution, erase uncertainty as a generic failure, or automatically retry coding.
- [ ] The integration harness crashes before creation, after creation, before result persistence and after persistence, including lost acknowledgements. Assert surviving child counts, same-request results and blocked replacements until reconciliation.
- [ ] Test both surviving-child and no-survivor outcomes, plus reboot-style restoration with no live child. Durable history and working files remain intact; native conversation recovery remains a separate explicit choice.
- [ ] Document crash windows, reconciliation evidence and residual limitations in line with the accepted uncertain-launch ADR.
