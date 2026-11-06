# 08: Retain readable session history

Status: ready-for-agent
Blocked by: 06
Resolved: yes

**What to build:** Read a stopped session’s prompts, responses and captured terminal output after restart, independently of the original native conversation or checkout.

- [x] A single supervisor-side persistence worker stores versioned SQLite metadata and bounded append-only transcript chunks in macOS application support, outside repository files, with stable task/session/request attribution and no copied credentials.
- [x] A stopped session is discoverable and its retained history can be read without a live child or native resume. Redraws, terminal controls and alternate screens have a readable representation; viewing history cannot execute captured terminal controls.
- [x] History remains accessible when its checkout is missing. Selection and lifecycle records survive restart; restoring records alone never restarts coding.
- [x] Acknowledge saved metadata only after durable success. Under healthy storage target durable transcript flushes at least once per second; clean shutdown flushes pending history and visibly reports write failures.
- [x] Test initial schema creation/migration, clean restart, abrupt failure, missing checkout, safe readable replay and recovered durable prefix through the integration seam. Distinguish recoverable committed data from unflushed output that may be lost.
- [x] Define finite chunk, buffering and in-flight persistence limits, with blocking storage work outside the UI. Storage fault hardening follows separately; no automatic deletion is introduced.
- [x] Record the actual durability boundary and preliminary flush observations, and update history/quit instructions. Do not present buffered writes as a durable acknowledgement.

## Comments

Implemented the single supervisor-side SQLite worker, transactional schema 0→1→2,
stable metadata/outcomes, bounded raw chunks/capture queue, periodic file-sync then
SQLite-commit acknowledgements, safe chronological history paging/scrolling and
restart restoration without coding. Quit waits for EOF and final flush and stays
visible on a real readonly-directory write failure; repair and explicit retry
recover the retained output. No raw input, credentials/config copies or automatic
history deletion are added.

[Proof and actual durability/limits](../../../docs/proofs/08-durable-history.md).
Real dashboard/supervisor/PTY tests cover clean/missing-checkout restart, abrupt
acknowledged-prefix recovery, safe erased/alternate-screen text, schema migration,
6000 ordered records across seven-plus bounded chunks, healthy measured commits,
and failed final storage/retry. Full merged locked build/fmt/Clippy/all-target suite
passes. Native resume/launch crash-window reconciliation, full-volume hardening,
power-loss guarantees and sustained resource/latency gates remain their separate
tickets; the proof distinguishes their limits.
