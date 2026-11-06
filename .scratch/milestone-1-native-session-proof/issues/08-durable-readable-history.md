# 08: Retain readable session history

Status: ready-for-agent
Blocked by: 06

**What to build:** Read a stopped session’s prompts, responses and captured terminal output after restart, independently of the original native conversation or checkout.

- [ ] A single supervisor-side persistence worker stores versioned SQLite metadata and bounded append-only transcript chunks in macOS application support, outside repository files, with stable task/session/request attribution and no copied credentials.
- [ ] A stopped session is discoverable and its retained history can be read without a live child or native resume. Redraws, terminal controls and alternate screens have a readable representation; viewing history cannot execute captured terminal controls.
- [ ] History remains accessible when its checkout is missing. Selection and lifecycle records survive restart; restoring records alone never restarts coding.
- [ ] Acknowledge saved metadata only after durable success. Under healthy storage target durable transcript flushes at least once per second; clean shutdown flushes pending history and visibly reports write failures.
- [ ] Test initial schema creation/migration, clean restart, abrupt failure, missing checkout, safe readable replay and recovered durable prefix through the integration seam. Distinguish recoverable committed data from unflushed output that may be lost.
- [ ] Define finite chunk, buffering and in-flight persistence limits, with blocking storage work outside the UI. Storage fault hardening follows separately; no automatic deletion is introduced.
- [ ] Record the actual durability boundary and preliminary flush observations, and update history/quit instructions. Do not present buffered writes as a durable acknowledgement.
