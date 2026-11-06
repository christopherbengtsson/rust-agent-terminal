# Milestone 2 coordination — agreed scope

Interview date: 2026-11-06. Scope: Milestone 2, PRODUCT C1–C7. Product decisions
are settled and the user acknowledged the architecture's suitability for future
shared-workspace agents. The [specification](spec.md) is now synthesized; the
[ticket breakdown](ticket-breakdown.md) was approved by the user ("great");
all 19 tickets are published under issues as ready-for-agent. This document records planned
behavior, not implemented Milestone 2 capabilities.

## Sources and existing work

- [Conductor research](conductor-research.md): official documentation/changelog,
  known differences, and unknowns. Research before asking questions; use Conductor
  defaults where established and raise actual conflicts with existing scope.
- [Milestone 1 evidence](../../docs/proofs/18-milestone-evidence.md): completed
  version-scoped native session proof, including durable independent history,
  resume proof, ownership, reconnect, status and storage faults. Native versions:
  Claude Code 2.1.318 and Codex CLI 0.167.0. Physical reboot and the full release
  workload are not established by that proof.
- Current implementation is globally single-active-session with singleton current
  adoption. Multi-project/workspace registry, concurrent runtimes, creation,
  search, archive/removal and diff/editor flows still need implementation.

## Confirmed product decisions

### Projects, workspaces and chats

- Projects are registered existing local repositories. Discovery stays read-only;
  explicitly adopt existing worktrees. No dedicated cloning flow in this scope.
- Workspace is the product term for the stream of work previously called Task.
  It has working files, a branch and successive native chats. Its identity
  survives chat exit, new conversations, tool changes and renaming.
- Independent workspaces run concurrently. One active coding session per worktree
  remains the release limit; multiple simultaneous agents in one workspace are a
  future addition, explicitly accepted as deferred in Q21.
- Create and start is a combined explicit action, including a supplied first
  prompt when present. Create without starting is also available. This closer
  Conductor alignment supersedes the earlier mandatory separate Create/Start
  user steps. Creation, setup and launch retain distinct outcomes and recovery.
- Creation needs no manually entered title. Use a stable generated location and
  initial name, derive readable names from the work, and allow manual renaming.
- Normally obtain the latest configured remote base. If fetching fails, explain
  and offer explicit creation from locally available code, identifying the commit.
  Repositories without a remote can use a selected local base.
- Search workspace metadata and retained history across projects, including
  archived workspaces and missing checkouts. History hits open the matching chat
  and passage. This cross-project history scope is explicitly accepted; equivalent
  Conductor local global-transcript behavior was not established by research.
- Explicit native resume can target older as well as latest conversations with
  their original tool. Occupied worktrees offer attach; stop before resuming a
  different conversation. Unavailable resume preserves readable history.

### Diffs and editors

- Show changed files and readable diffs in the dashboard. PRODUCT requires Working
  changes by default and an explicit whole-workspace alternative containing earlier
  branch commits plus working edits; retain staged/unstaged/new-file distinctions.
- Follow Conductor's refreshed comparisons while preserving selected context.
  Withdraw the earlier manual-refresh-only dashboard proposal. Exact bounded
  refresh policy and external-editor snapshot semantics are engineering details.
- Open both comparisons in Vim or a configured editor; separately open real
  workspace files for editing, including the currently viewed source file.
  Terminal-editor exit restores the previous workspace/dashboard context.
- Editing directly inside dashboard diffs is a later addition, explicitly
  accepted as deferred in Q22. Exact external editor line positioning is an
  implementation detail, not a verified Conductor guarantee.

### Archive, save, restore and cleanup

- Stop and archive is a combined explicit action, requiring confirmed stop before
  saving/archive completion. This supersedes the earlier mandatory separate
  Stop/Archive user steps. Stop remains independently available.
- Archive hides active work and retains the folder/history. Unarchive restores
  visibility and a usable branch without starting coding.
- Save unfinished changes as a clearly named local Archive checkpoint commit,
  not an ordinary Git stash. The user accepted the extra history commit and loss
  of staged/unstaged separation in that commit after discussing the trade-off.
- Include tracked and new non-ignored files. Leave ignored files untouched in
  the retained folder and explain their exclusion. Unsupported/nested repository
  state must not be falsely reported as fully saved.
- Confirm saving before success feedback; identify the saved checkpoint and say
  locally saved, not pushed. Failed saving leaves work unarchived with an error.
- Q23 final answer: enable Delete local branch on archive by default for branches
  created by the product. Existing adopted branches are excluded unless separately
  selected for deletion. Never delete remote branches automatically.
- Preserve archived code through a recovery reference independently of the visible
  branch name before deletion. Restore can recreate a branch and must handle a
  reused old name without overwriting it. This latest decision supersedes earlier
  permanent branch retention, addressing stale branch accumulation.
- Removing the workspace folder remains separate. Retain code recovery, history
  and any branch not selected for archive deletion. Block unsaved files or uncertain
  checks; warn and explicitly confirm code saved only on this Mac before removal.
- History deletion is separate and explicit. Archive-ref/code retention is not
  permission for automatic transcript or code deletion.

### Setup and failure choices

- Optional configured project setup runs on creation, with visible progress and
  errors, following documented Conductor behavior.
- User delegated undocumented setup-failure details to best judgment: keep the
  workspace and offer Retry setup or Continue without setup. Agent launch needs
  explicit intent, including a previously requested Create and start; explain
  whether continuing proceeds with it. No silent coding launch after failure.
- Include configuration necessary for these working features; broader app-running
  actions, archive scripts and special file-copy configuration are not implied.

## Architecture and implementation obligations

- Extend the sole supervisor/persistence ownership and existing workers; keep
  blocking Git/process/filesystem work outside the UI loop and one Rust package.
- Model project/workspace/session identities separately, with a session collection
  per workspace. Enforce today's concurrency limit through admission policy,
  avoiding a singleton runtime/schema that would obstruct later shared-workspace
  agents. Reservations use canonical worktree identity, so duplicate workspace
  records cannot bypass exclusivity. No speculative plugin/provider/concurrency
  framework is requested.
- Separate comparison data/file attribution, rendering and external editor
  execution, so future inline editing can add an explicit freshness-checked save
  operation without replacing diff discovery.
- Preserve durable acknowledgements, same-request result recovery, uncertain-launch
  reservations, transcript retention and original native conversation identity.
  Migrate retained Milestone 1 records without losing history or restarting coding.
- Record recoverable creation/save/ref-deletion/removal operations and partial
  outcomes, attribute replies by stable identity, reconcile external Git changes
  and never turn a missing acknowledgement into duplicate execution.
- Establish a native-compatible automatic naming mechanism for both tools; native
  title extraction is unproven. Do not introduce an undisclosed extra AI provider
  or label a screen-text guess as a native title. Any feasibility change to the
  accepted experience returns for a decision with evidence.
- Define comparison revisions, bounded search/results, history-deletion/index
  consistency, file coverage, conflicts/binaries/submodules, editor failures,
  archive-ref retention/restoration and unsupported-case errors in the spec.
- Validate with real temporary Git repositories and the existing dashboard/
  supervisor/scripted-PTY seam, plus actual Claude, Codex and editor checks on
  macOS. Exercise multiple projects/concurrent workspaces, crash/lost-reply/storage
  cases and C6 reboot evidence. Do not relabel Milestone 1 proof as new validation
  or claim the later combined GitHub/reviewer workload is already demonstrated.

## Milestone 1 implementation audit: prerequisites within Milestone 2

User requested comparison of actual completed Milestone 1 implementation with
Conductor, including significant differences as prerequisites where appropriate.
This is a read-only source audit; no new runtime checks were run. Treat the
following as early implementation/qualification tickets within Milestone 2,
rather than reopening or relabeling the completed native-session proof.

1. **Durable project/workspace registry and targeted runtimes.** App and supervisor
   have one current observation/checkout/child. SQLite stores current adoption in
   singleton app state; adopting another checkout creates a new task ID. Historical
   session rows are not a complete workspace registry. Migrate existing identities
   and history into retained project/workspace/session records before adding
   multi-workspace navigation. Sources: src/app.rs:11; src/session.rs:319, :923;
   src/persistence.rs:349, :760.
2. **Stable target identity on every operation.** Input, resize, Stop, Resume and
   reconciliation implicitly target the current runtime. History requests use row
   offsets that can change as sessions arrive or disappear. Scope commands and
   replies to stable workspace/session/conversation IDs so changing selection
   cannot redirect queued input/stop or open/resume the wrong conversation.
   Sources: src/events.rs:47; src/session.rs:847, :963;
   src/persistence.rs:871.
3. **Fair multi-session capture and search scheduling.** The sole writer keeps
   one active transcript handle and flushes/closes it on session-ID changes.
   Interleaved streams would force frequent commits, and progress/final-flush
   coordination is singleton. Retain the sole writer but add bounded per-session
   batching/accounting and control fairness. Search/indexing must interleave with
   capture and respect committed extents/deletion. Sources:
   src/persistence.rs:178, :792; rat-persistence/references/search.md.
4. **Execution-owned, bounded Git jobs and recoverable mutations.** Discovery is
   dashboard-local and adoption synchronously resolves paths/Git in the supervisor
   request loop. Move potentially blocking checkout work into execution-side
   workers. Define per-repository serialization, timeouts, cancellation, stable
   mutation request outcomes and Git/SQLite reconciliation before create/archive/
   delete features depend on them. Sources: src/main.rs:111; src/git.rs:37;
   src/session.rs:901; architecture execution-machine boundary.
5. **Native workflow/history/naming qualification early.** Current history is
   2048-byte raw-output paging with lossy UTF-8 conversion and escaped controls,
   not structured chat messages; redraw artifacts and split-character boundaries
   affect readable search. Input is not independently recorded; prompts appear
   when the native CLI echoes them. Establish useful text projection/search
   snippets and Create-and-start/automatic naming with both real native tools
   before building UI on assumed structured conversations. Preserve raw history
   independently; do not capture arbitrary keys/passwords or silently require
   importing all native history. Sources: src/persistence.rs:871–918;
   src/session.rs:1198–1239. Conductor's chat search/automatic chat titles are
   documented in conductor-research.md.
6. **Compatibility and multi-workspace attention checks.** Native semantic support
   is qualified against exact recorded versions/layouts; other versions may launch
   with Unknown. Input/approval evidence expires around 30 seconds and working
   evidence around five. Test coordination after leaving a workspace for minutes,
   retaining any last attention event as explicitly historical context instead of
   extending unsupported current-state confidence. Surface supported tool/version
   limits and explicit launch configuration for a long-lived supervisor's
   inherited PATH/environment. Sources: src/native_status.rs:188, :452, :461;
   src/codex_status.rs:141, :281; src/session.rs:65–93; ADR 0003.

Keep PTYs, supervisor ownership, durable launch/recovery gates and sole-writer
persistence as the established foundation. No evidence justifies replacing them
to mimic Conductor's implementation. Follow with the agreed product features and
fresh multi-project/concurrent native/resource/reboot/editor checks. Native CLI
presentation, one active agent per workspace and external source editing remain
accepted differences; shared-workspace agents and inline editing are later work.

## Cross-milestone prerequisite audit

User requested checking the prerequisites against future milestones. The audit
found no need to move GitHub reviews, own-PR actions, notification delivery or
combined release validation into Milestone 2. It did find and correct one actual
wording mismatch: reserve canonical worktree identity, not merely workspace ID.
Otherwise two workspace records could bypass the release's checkout exclusivity.

The authoritative planned boundaries are now in
[architecture: later-milestone compatibility](../../docs/architecture.md#compatibility-with-later-milestones).
Milestone 2 prerequisites remain early tickets within the existing milestone.
They must preserve independent reviewer identity/restrictions, separate structured
draft storage and review comparisons, branch-independent future PR identity,
separate current status/historical attention, and explicit launch/setup intent.
Partial archive outcomes remain distinguishable without losing recovery refs.
No runtime tests were run for this documentation/source audit.

## Architecture exploration before specification

User invoked improve-codebase-architecture scoped to existing code Milestone 2
will extend, then delegated technical judgment after asking about future agents
sharing a workspace. A fresh source scan found launch progression, history
completion and worktree operations as strong deepening candidates. Native input
acceptance/status ordering is worth investigating, not a reproduced defect.

Three independent launch designs were compared: a small isolated launch
transaction, an explicit launch protocol/executor, and a concrete Agent session
module with private launch progression. Recommend the latter as the target shape:
one session owner spans provisional launch, live execution and final capture,
while private modules retain cohesive launch/execution/history responsibilities.
The supervisor routes and observes; workspace admission and shared Git state stay
outside this seam. This supports future shared-workspace agents without creating
another singleton runtime per workspace.

Important finding: a native screen can already produce output before launch
result durability. A separate launch module cannot delay runtime ownership and
native input/control until a success-only resource handoff. Keep ownership from
spawn onward, with provisional execution and durable outcome observed separately.

Recommended early implementation slices, preserving behavior and existing proofs:

1. Attribute storage success AND failure to exact session, launch request where
   applicable, operation identity and purpose. Untagged Saved/Error or a global
   save counter cannot prove one launch's outcome durable. Reject stale snapshot
   overwrites and unrelated completions rather than advance another attempt.
2. Retain a successfully spawned child immediately, before later fallible setup.
   Cancellation/failed setup must retain cleanup ownership; this is a structural
   risk found in source, not a demonstrated process leak.
3. Establish the concrete per-session runtime and private launch phase progression
   incrementally; retain existing native/process/storage implementations, one
   supervisor/writer/package and bounded nonblocking dispatch.
4. Retain lost-ack, crash-window, stalled-prepare Stop, receipt-gap and pending-
   capture scenarios; add cross-session/stale completion and partial-spawn cases.
   Check protocol compatibility with an already-running older supervisor before
   evolving messages; never start a competing owner or migrate under its writer.

Native input/status deepening is conditional on establishing the intended evidence
invalidation policy when input is rejected. Leave proven lifecycle termination,
recovery evidence and history deletion implementations intact. No signatures or
generic framework are committed by this exploration; no runtime code changed and
no native tests were rerun. These recommendations refine the prerequisite tickets
rather than introduce a separate broad refactor milestone.

## Scope boundary and specification

User explicitly requested documenting two later Conductor-alignment additions:
Open GitHub project (select/clone a GitHub repository) and Quick start (create a
new repository). Both are now recorded in PRODUCT's future possibilities, with
no delivery milestone assigned. Milestone 2 keeps Open project for repositories
already on this Mac; do not add cloning/new-repository controls to its tickets.

Close Conductor alignment governs the workflow within Milestone 2. Explicit
exceptions: one active agent per workspace and external source editing, with
future migration boundaries documented in architecture. Keep local macOS/native
CLI behavior and PRODUCT C1–C7. GitHub review preparation/submission, own-PR
feedback/merging, remote/cloud execution, dedicated cloning and unrelated release
setup/notifications remain later work.

Source of product behavior: PRODUCT.md. Domain terms: GLOSSARY.md. Archive
trade-off and branch cleanup: ADR 0007. The spec carries forward the previously
approved dashboard/supervisor/scripted-PTY testing seam, real temporary Git
repositories and actual native/editor checks. The 19 tickets are published as ready-for-agent, with completion tracked
separately. No runtime implementation has started.
