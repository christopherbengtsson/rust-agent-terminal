# 02: Retain ownership through partial spawn failure

Status: ready-for-agent
Resolved: yes
Blocked by: None

**What to build:** A cancelled or failed launch never abandons a process merely because setup failed after spawn.

- [x] Install process/resource ownership immediately after successful spawn, before subsequent fallible setup. Keep gate, scope, PTY and capture obligations owned until transferred or confirmed cleaned.
- [x] Exercise failures after creation and before input/capture/authorization installation, plus cancellation while creation is outstanding, with actual gated fixture processes.
- [x] Never release a cancelled gate. Unconfirmed cleanup preserves a retryable reservation and blocks replacement rather than dropping the child or inventing success.
- [x] Keep Stop responsive during storage trouble, retain original native exit outcomes and preserve current descendant verification. Do not replace proven lifecycle or recovery implementations.
- [x] Preserve ordinary launch, native argument/environment/cwd construction, missing executable and quit behavior. Record fixture checks separately from native compatibility.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.

Implementation checkpoint: `a877c48`, `3d5b216`, merged with attributed durable
operations at `6e1f7f8` (integration `2b2a255`). Immediate child/PTY/gate ownership,
bounded asynchronous creation, cancellation and retained cleanup reservations
are implemented. The four new real-process regressions pass after that merge:
five setup failure boundaries, actual verification failure/repair, cancellation
while a gated child is still owned by the creation worker, and a restart check
preventing prior-session EOF proof from sealing an uninstalled capture.
Locked build, formatting, warnings-denied all-target Clippy and diff checks pass.
Full validation details and the intermediate full-suite failures/corrections are
in [the fixture proof](../../../docs/proofs/m2-02-partial-spawn-ownership.md).
Native interaction qualification is not inferred from fixtures. Resolution awaits
the coordinator's serial final integration all-target check.


Integration resolution, 2026-11-06: acceptance behavior is demonstrated by the
combined real-process union run and focused receipt-framing correction check.
See [integration evidence](../../../docs/proofs/m2-prerequisite-integration.md).
The initial all-target command was not green: its sole ownership failure was a
strict fixture-ledger JSON parse, corrected without changing capture assertions.
The complete post-correction all-target rerun is active and pending; this ticket
resolution records demonstrated scoped acceptance, not an aggregate gate pass or
new native CLI compatibility qualification.
