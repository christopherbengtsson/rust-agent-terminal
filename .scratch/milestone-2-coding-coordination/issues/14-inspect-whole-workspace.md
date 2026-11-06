# 14: Inspect prior commits and current workspace changes

Status: ready-for-agent
Resolved: yes
Blocked by: 13

**What to build:** Switch explicitly to Whole workspace and see branch work from its selected base alongside current edits.

- [x] Use common-ancestor comparison against the selected local base, include subsequent branch commits and current working edits, and identify the actual compared commits.
- [x] Persist selected local base ref/commit for adopted workspaces independently of current branch/ref and reuse creation metadata where present. Missing/ambiguous/unrelated histories require explanation or selection; the slice is demonstrable on adopted branches without workspace creation.
- [x] Inspection itself does not fetch or remember Whole workspace as the next default. Base movement, branch switches and refreshed comparisons remain attributable and visible.
- [x] Verify earlier committed changes plus overlapping staged/unstaged/new files, moved bases, rename/deletion/binary/conflict cases and externally changed refs using real repositories.
- [x] Keep coding comparisons distinct from future pinned review head/base comparisons; archive refs are not silently substituted as review bases.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Implemented on isolated branch `m2-14-whole-workspace` from integration `5c1a604`,
including corrected compatibility checkpoint `c20cd49`. Whole workspace has an
explicit `w` switch and `b` local-base selection; durable ref/selection commit,
actual HEAD/base/common-ancestor labels, no fetch or index mutation, and default
Working reopening are implemented. Creation's initial base seed is coordinated
with ticket 09 independently of its immutable original-base facts.

[Evidence](../../../docs/proofs/m2-14-whole-workspace.md) records actual real-Git,
Controller durability/restart/fault checks, 160×40 and 60×18 dashboard captures,
and protocol-1 live-owner refusal. Build, formatting, warnings-denied all-target
Clippy, ten diff tests, twenty-one App and thirteen UI checks pass. The parent
owns the final aggregate gate. Editor/native/release-workload and unsupported
filesystem limitations remain explicit in the proof; ticket 15 is separate.
