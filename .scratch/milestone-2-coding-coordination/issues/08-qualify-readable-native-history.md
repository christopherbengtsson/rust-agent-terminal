# 08: Qualify readable native session history

Status: ready-for-agent
Resolved: no
Blocked by: 03, 05

**What to build:** Open retained history as useful readable text with stable passage locations, independently of native resume and automatic naming.

- [ ] Provide a safe readable history view from committed captured output while retaining raw bytes and identities independently; never execute captured terminal controls.
- [ ] Handle UTF-8 and terminal control/redraw sequences across capture/page boundaries with bounded work and stable raw-to-readable passage locations. Explain irreducible terminal artifacts rather than promise structured chat reconstruction.
- [ ] Demonstrate recognizable prompts, answers and command output with actual Claude and Codex on the existing adopted-workspace launch path; input is retained only through appropriate visible/native evidence, not arbitrary key/password logging.
- [ ] Retain useful readable passages after restart, missing checkout or unavailable native conversation; respect committed extents, deletion tombstones and per-session ownership.
- [ ] Record actual native versions and history examples, projection limits and focused fixture cases. This ticket supplies the readable view/projection used by search; global indexing/search scheduling belongs to ticket 11.

## Comments

Verify this path through the established real-state/fixture seam and actual
native captures. Native prompt delivery and naming are qualified in ticket 07.
