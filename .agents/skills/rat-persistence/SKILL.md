---
name: rat-persistence
description: Change SQLite persistence, history storage/search/deletion, or storage recovery in this project. Use for storage I/O and durable acknowledgements; model-only serialization stays with its feature skill.
---

# Durable product state

Paths are repository-relative.

1. Locate the affected operation and completion in `src/persistence.rs`. For
   schema/driver changes, inspect `Store::open` and `Cargo.lock`. Read
   [ADR 0004](../../../docs/adr/0004-preserve-history-with-explicit-cleanup.md)
   for the durability and retention contract; identify the record and restart
   result the change must preserve.
2. Use the existing sole worker interface; extend it when the operation needs
   a new command or completion. Select the branch below before changing write,
   migration, search or deletion behavior. A bounded read-format change needs
   its read/render path, not a new durability protocol.
3. Exercise the affected on-disk boundary in temporary state using
   `tests/ownership.rs`. Verify content and identities, including restart/fault
   cases when the change affects them. Report demonstrated recovery and remaining
   loss windows; distinguish accepted, written and durable output in evidence.

## Boundaries

- **Storage ownership:** when changing state-directory selection, storage
  placement or recoverable request outcomes, read the
  [execution-machine boundary](../../../docs/architecture.md#execution-machine-boundary-direction-local-execution-today).
- **Writes/migrations:** inspect `Store::open`, `append`, `flush` and their
  completion consumers. Read [durable-history evidence](../../../docs/proofs/08-durable-history.md)
  for transcript-file/SQLite ordering, committed extents and older-schema recovery.
  Validate the relevant failure boundaries before advancing saved-state UI.
- **Fault/retry/backpressure:** read
  [storage-fault evidence](../../../docs/proofs/14-storage-faults.md) for partial
  writes, retained blocks, directory sync, stalls and error clearing. Use
  `$rat-sessions` when changing Stop, EOF or final-capture coordination.
- **History reads/search:** inspect `Store::history` and the existing bounded-page
  tests. Preserve escaped display and committed-extent reads. For a new search,
  read [search scheduling](references/search.md) before choosing the query/index
  execution path.
- **Deletion:** inspect `src/cleanup.rs` and the storage tombstone path. Read
  [cleanup evidence](../../../docs/proofs/15-explicit-history-cleanup.md) for
  identity-bound intent, eligibility, inode anchoring, symlink handling and crash
  completion. Reuse that path rather than deleting saved paths directly.

For driver/sync changes, consult the locked Rusqlite API and
[SQLite PRAGMAs](https://www.sqlite.org/pragma.html). Changes to journaling,
connections or sync settings require recovery evidence for this file-plus-database
contract; generic SQLite tuning does not establish it.
