# 09: Reconcile launches after supervisor failure

Status: ready-for-agent
Blocked by: 07, 08
Resolved: yes

**What to build:** After supervisor failure or reboot, understand interrupted work and prevent replacement coding from competing with an uncertain surviving child.

- [x] Persist launch intent and outcomes with stable request/session identities so failure around process creation restores interrupted or uncertain records instead of silently relaunching.
- [x] Reconcile whether a child survived before releasing worktree exclusivity. Unresolved identity or survival blocks a replacement launch with a visible explanation and explicit recovery path; a reused PID alone is insufficient evidence.
- [x] Reconnect recovers known outcomes for the same request across supervisor restart. Do not claim exactly-once execution, erase uncertainty as a generic failure, or automatically retry coding.
- [x] The integration harness crashes before creation, after creation, before result persistence and after persistence, including lost acknowledgements. Assert surviving child counts, same-request results and blocked replacements until reconciliation.
- [x] Test both surviving-child and no-survivor outcomes, plus reboot-style restoration with no live child. Durable history and working files remain intact; native conversation recovery remains a separate explicit choice.
- [x] Document crash windows, reconciliation evidence and residual limitations in line with the accepted uncertain-launch ADR.


## Comments

Implemented durable Prepare/Authorize completions, same-executable fail-closed
native exec gate, retained request outcomes, and explicit `u` reconciliation.
macOS boot-session UUID plus microsecond process start identity distinguish a
survivor from a reused PID. No restored PID is signaled. Never-authorized coding
can reconcile immediately; authorized native execution whose descendants cannot
be excluded on the same boot requires verified reboot. Legacy uncertainty uses
a durable first-recovery observation-boot anchor so a later reboot can release it
without inventing historical launch evidence. Interrupted history remains intact;
new conversation launch is always explicit.

The real dashboard/supervisor/PTY harness covers four crash windows, lost ack,
actual surviving child, gate-only no-coding outcomes, injected later-boot evidence
after known fixture cleanup, failed native exec replay, unchanged working files,
readable history, and one-child explicit replacement. This does not claim a
physical reboot, exactly-once execution, arbitrary descendant cleanup or native
conversation resume. See [the proof](../../../docs/proofs/09-launch-reconciliation.md).

Latest integration `ac3ea5a` merged; full stable locked build/fmt/Clippy/tests
passed (20 unit, 25 ownership/history/recovery, scripted and screen integrations;
one subprocess probe ignored directly and driven by terminal restoration).
