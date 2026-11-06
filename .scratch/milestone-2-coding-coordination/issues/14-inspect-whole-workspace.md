# 14: Inspect prior commits and current workspace changes

Status: ready-for-agent
Resolved: no
Blocked by: 13

**What to build:** Switch explicitly to Whole workspace and see branch work from its selected base alongside current edits.

- [ ] Use common-ancestor comparison against the selected local base, include subsequent branch commits and current working edits, and identify the actual compared commits.
- [ ] Persist selected local base ref/commit for adopted workspaces independently of current branch/ref and reuse creation metadata where present. Missing/ambiguous/unrelated histories require explanation or selection; the slice is demonstrable on adopted branches without workspace creation.
- [ ] Inspection itself does not fetch or remember Whole workspace as the next default. Base movement, branch switches and refreshed comparisons remain attributable and visible.
- [ ] Verify earlier committed changes plus overlapping staged/unstaged/new files, moved bases, rename/deletion/binary/conflict cases and externally changed refs using real repositories.
- [ ] Keep coding comparisons distinct from future pinned review head/base comparisons; archive refs are not silently substituted as review bases.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
