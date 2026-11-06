# 04: Resolve worktrees through bounded execution-owned jobs

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** Discovery, refresh and adoption resolve actual checkouts without blocking navigation or shared session controls.

- [x] Concentrate existing discovery and adoption Git/path interpretation in execution-owned workers. Preserve original Unix path bytes, NUL-safe parsing and canonical worktree identity.
- [x] A slow or failed Git process is bounded/cancellable, leaves prior successful discovery visible, and cannot block Stop, session polling or dashboard exit.
- [x] Route results by request/project/checkout identity so late refresh/adoption replies cannot retarget the current selection. Keep discovery read-only and adoption explicit.
- [ ] Verify actual temporary linked, aliased, detached, bare, locked, missing and prunable worktrees, including non-UTF-8 paths and saturated dispatch.
- [x] Establish a concrete per-repository mutation execution discipline for subsequent creation/archive work without holding exclusion for whole agent sessions or creating a generic job engine.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.

Implementation prepared on `ticket/04-worktree-jobs`:

- Discovery now uses attributed supervisor requests/observations; adoption resolves
  canonical root, worktree Git directory and common directory in a separate worker.
  Byte-preserving wire fields never round-trip through display strings.
- Git children have two-second deadlines, bounded nonblocking output, cancellation,
  owned process-group cleanup and bounded dispatch. Stop cancels jobs, and supervisor
  exit waits for their cleanup without blocking its control loop. A refused second
  dashboard retains read-only discovery through a restricted execution helper.
- Added a concrete cancellable per-canonical-common-directory mutation lease for
  subsequent creation/archive workers; it is not held for agent lifetimes.
- Passing checks so far: locked build, formatting, warnings-denied all-target
  Clippy, 32 bin tests (one existing subprocess probe ignored), focused real
  supervisor tests for slow/saturated Git controls, legacy live-owner negotiation,
  refused-dashboard discovery, aliases/repeated launch, explicit cleanup, and
  unavailable state-directory handling.
- The first all-target run exposed readiness assumptions (combined adoption/launch
  and discovery without a usable owner). Focused fixes passed; that already-failed
  run was interrupted after it stopped progressing during concurrent host PTY runs.
  A final serial integrated all-target run is still required before resolution.
- Actual linked/detached/locked/prunable/bare/missing/alias repositories are covered.
  This native macOS filesystem rejects non-UTF-8 directory names with EILSEQ; the
  test explicitly records that limitation, verifies unavailable-path rejection,
  and exercises byte-exact parser/wire round trips. A real non-UTF-8 checkout on a
  filesystem supporting those names remains unverified here. No new Claude/Codex
  native compatibility claim is made.

Merged integration prerequisite `2b2a255` into the ticket branch and adapted the
asynchronous adoption save to `Obligations::send`, preserving exact operation
attribution and transient metadata exclusions. Post-merge warnings-denied Clippy,
32 bin tests, locked build/formatting and focused slow/saturated Git plus
refused-dashboard discovery tests pass. The final all-target run is coordinated
serially on the integration branch; this ticket intentionally remains unresolved
until that evidence is recorded.
