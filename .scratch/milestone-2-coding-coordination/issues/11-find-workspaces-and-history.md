# 11: Browse and search retained workspaces and conversations

Status: ready-for-agent
Resolved: yes
Blocked by: 06, 08

**What to build:** Find work by metadata or words in saved history across projects and open the selected chat at its matching passage.

- [x] Search project/workspace titles, branches and paths plus committed readable conversation/output text. Include archived/missing records with clear labels and useful bounded snippets.
- [x] List retained sessions/conversations by stable identity; opening a row/search hit is not implicit native resume and cannot be redirected by new sessions, reordering or deletions.
- [x] Bound/cancel search/indexing and attribute results to the current query. Interleave with capture/durable metadata; measure healthy flush delay during active output and search.
- [x] Recover searchable projections after restart, preserve UTF-8/control safety and committed-prefix rules, and remove explicitly deleted transcript content from results without deleting retained recovery identity.
- [x] Verify keyboard selection, no-results/errors, stale query replies, archived/missing folders, concurrent capture and deletion using the established real-state/fixture seam.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Implemented and scoped-qualified on source `8844b88309cf13b8cff3f349474eb851e8fee382`.
See [the retained-search evidence](../../../docs/proofs/m2-11-retained-search.md)
for exact source/binary, query/index bounds, stable passage identity, deletion and
restart boundaries, actual normal/small keyboard proof, native08 corpus search
replays and the source-pinned active-output fixture measurement. That run completed
65 searches (max 53 ms) alongside 16 contiguous durable receipts (max accepted age
289621 µs). These are short fixture results, not native or full milestone workload
claims. Reading/search never launches or implicitly selects native Resume.

All five ticket 11 requirements are scoped complete. Archive metadata is a minimal
schema seam consumed by search; archive actions remain ticket 16. Native selected-conversation
resume remains ticket 12. Final assembled migration/native/resource/reboot/all-target gates
remain the root milestone qualification; previously documented aggregate boundaries
are not relabelled passed. No GitHub PR/review publication occurred.
