# 05: Register projects and retain workspace identity

Status: ready-for-agent
Resolved: yes
Blocked by: 01, 04

**What to build:** Add existing local repositories, navigate their discovered/adopted workspaces and reopen the dashboard to the same retained work and history.

- [x] Persist projects, workspaces and session membership independently of selection, editable names, current branch and folder availability. Repeated adoption selects the retained workspace.
- [x] Migrate existing adoption, opaque legacy task/request identities, native conversation IDs, request outcomes and transcript associations without deletion, guessed identity or automatic coding restart.
- [x] Deduplicate aliases/linked-checkout registration using repository facts; enforce checkout identity independently of display labels. Missing projects/folders remain visible and searchable later.
- [x] Provide keyboard project/workspace navigation, loading/error states and usable small-terminal selection; preserve reference composition and honest unavailable reviews.
- [x] Keep an existing supervisor and sole writer authoritative during version/schema transition; reject unsupported live protocol rather than launch a competing owner. Verify migration/restart and failure cases.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.

2026-11-06 implementation: durable schema-4 registry and opaque legacy mappings,
canonical project/checkout deduplication, retained session membership, explicit
project registration and normal/small keyboard workspace navigation are working.
Migration/rollback, restart, aliases/linked worktrees, non-UTF-8 legacy paths,
missing facts, registration failure/retry and unsupported live-owner refusal
passed focused real-supervisor checks. Real dashboard normal/small PTY selection
and captured cells passed; App/UI checks passed. See
[exact evidence and limitations](../../../docs/proofs/m2-05-registry.md).
The final combined all-target regression gate remains the integration merger's
responsibility; no fresh native-tool or concurrent-session proof is claimed.
