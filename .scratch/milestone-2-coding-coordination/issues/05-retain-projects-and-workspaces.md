# 05: Register projects and retain workspace identity

Status: ready-for-agent
Resolved: no
Blocked by: 01, 04

**What to build:** Add existing local repositories, navigate their discovered/adopted workspaces and reopen the dashboard to the same retained work and history.

- [ ] Persist projects, workspaces and session membership independently of selection, editable names, current branch and folder availability. Repeated adoption selects the retained workspace.
- [ ] Migrate existing adoption, opaque legacy task/request identities, native conversation IDs, request outcomes and transcript associations without deletion, guessed identity or automatic coding restart.
- [ ] Deduplicate aliases/linked-checkout registration using repository facts; enforce checkout identity independently of display labels. Missing projects/folders remain visible and searchable later.
- [ ] Provide keyboard project/workspace navigation, loading/error states and usable small-terminal selection; preserve reference composition and honest unavailable reviews.
- [ ] Keep an existing supervisor and sole writer authoritative during version/schema transition; reject unsupported live protocol rather than launch a competing owner. Verify migration/restart and failure cases.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
