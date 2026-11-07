# 18: Retain submitted history and handle renewed requests

Status: ready-for-agent
Resolved: no
Blocked by: 17

**What to build:** Keep confirmed review history accessible while completed cycles leave the active inbox, and distinguish a renewed personal request from continued monitoring of completed work.

**Acceptance coverage:** R8; R9 retained drafts.

## Acceptance criteria

- [ ] Confirmed submission removes its cycle from the active inbox and retains the reviewed/validated comparison, outcome and exact submitted comments, without deleting prior draft context.
- [ ] Uncertain publication stays visibly unresolved and does not masquerade as a completed cycle. Reconciliation updates history only on confirmed evidence.
- [ ] Do not actively monitor completed reviews for later commits or trigger change notifications/automatic regeneration. Explicit reopening refreshes current state and shows changed-since-review information.
- [ ] Continue discovering renewed personal requests using the proven request-cycle semantics. Renewals at either the same or a new revision create fresh cycles rather than reviving submitted comments as pending suggestions.
- [ ] Closed, merged or withdrawn requests remain absent from preparation eligibility while saved drafts/history stay accessible.
- [ ] Verify submit/reopen/renew sequences, missed-observation cases from the proof, same-revision renewals, restart and source-identity changes through the workflow seam; include real GitHub observation evidence where available.

## Scope and handoff

Completed review history is independent of coding workspace/archive/folder lifetime. This does not implement own-PR feedback, notifications or thread replies.

## Comments
