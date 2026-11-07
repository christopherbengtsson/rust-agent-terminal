# Architecture sketch

The [vision](../VISION.md) explains the direction and [PRODUCT.md](../PRODUCT.md)
defines release behavior. Start with one Rust package and module boundaries;
introduce separate crates only when a demonstrated need warrants them. The
[milestone plan](milestones.md) separates the current foundation from the session
prototype and subsequent GitHub review work.

## Boundaries and data flow

The UI works toward the [dashboard image](images/conductor-dashboard-v3.jpg):
reviews and project navigation on the left, a prominent native-session view on
the right, and visible status and keyboard actions. Adapt to terminal size and
validate with screenshots as usable slices land. Surface feasibility problems
and material deviations explicitly; product behavior, including one active
session per worktree and native CLI interaction, governs conflicts in the image.

**Terminal and UI (`terminal`, `ui`, `main`).** Crossterm owns raw mode and the
alternate screen; Ratatui renders a snapshot of application state. A terminal
guard restores modes on errors and normal exit, with panic restoration as well.
The event loop polls keyboard/resize input, consumes a bounded batch of worker
events, applies state transitions, and draws. It never waits for Git, process
output, agent startup, disk writes, or network calls. Polling currently waits at
most 50 ms for input; future streaming events must have per-frame budgets so a
busy session cannot starve navigation. This is a design budget, not a measured
claim against the full product workload.

**Application state and events (`app`, `events`).** State owns the discovered
worktrees, selection, refresh state, and errors. Input maps to actions; worker
results arrive as typed events. Refresh retains selection by worktree path and
retains old data on failure. Rendering does not execute effects. Session/task
identity, focus, session lifecycle, review revisions, and user decisions will
extend this model. A process being alive is not proof that an agent is working;
quiet output is not proof of completion. Report unknown semantic state explicitly
until a supported adapter supplies evidence.
Supported native-tool versions must demonstrate dependable lifecycle and
attention signals in macOS scenarios; see
[the status decision](adr/0003-evidence-based-agent-status.md).

**Git/worktrees (`git`).** The supervisor accepts attributed `Discover` requests
and owns a dedicated worker that runs `git -C <path> worktree list
--porcelain -z`, parses NUL-delimited fields, and sends a completion event.
Arguments are passed directly to Git, without shell interpolation. Discovery is
read-only, handles linked/detached/bare/locked/prunable worktrees, and preserves
non-UTF-8 Unix paths internally. The bounded request queue and one in-flight
refresh prevent repeated keystrokes spawning processes. Errors remain visible
and retryable. Each Git child has a two-second deadline, nonblocking bounded output
(256 KiB per pipe), and an owned process group that is killed/reaped on completion,
error or cancellation. A full result channel cannot strand cancellation. Stop and
shutdown cancel pending discovery/adoption without waiting on Git; stale replies
cannot change App's project/selection or complete a cancelled adoption. Adoption
resolves canonical root, worktree Git directory and shared common directory in a
worker, returning byte-preserving identity separately from display labels.

`git::RepositoryMutation::acquire` provides concrete per-common-directory mutation
exclusion for creation/archive workers. Acquire only inside execution workers,
recheck identity and safety before invoking Git, and release immediately after the
actual mutation/rechecks. It is process-local under the sole supervisor, has a
bounded cancellable wait, and is never retained for an agent lifetime. Git's own
ref/checkout refusals remain authoritative against external actors. Creation uses this exclusion for fetch and checkout mutation, releasing it before
writer acknowledgements or native launch; discovery/adoption remain read-only.

**Agent sessions (target; scripted prototype below).** A local supervisor process, implemented as another
mode of the same executable, must own child processes, PTYs, and history. The
dashboard is a reconnectable client over a user-private local socket. There is
one supervisor per macOS user, shared across projects, and at most one connected
dashboard controls it at a time; see
[the ownership decision](adr/0001-user-wide-supervisor.md). This separation is
necessary because the product requires agents to survive terminal
or dashboard loss. The scripted prototype below implements supervisor/session
ownership, durable persistence and conservative explicit crash reconciliation.
The supervisor enforces one active or uncertain execution per canonical worktree
and owns transport, controller election and Git admission. Its bounded session
collection preserves each concrete `session::agent_session` owner, which retains
launch deduplication, stable session identity, provisional execution,
native controls, lifecycle transitions and final capture/durability obligations.
The owner is a Rust value advanced by the supervisor, with private launch,
execution and history records; it adds no session thread or event bus. It returns
an observation snapshot and derived execution/capture reservation, without exposing phase
flags. The sole storage worker's completions are drained once and dispatched to
the owner holding the matching operation obligation. Existing bounded creation,
input, receipt, capture and cleanup workers retain their roles. Ordinary quit
requests supervisor shutdown, which combines Git-job completion with every
session owner's confirmed shutdown; an unexpectedly
lost client does not. On
supervisor failure/reboot, saved sessions become interrupted and require explicit
recovery, never automatic coding restarts. Claude Code and Codex adapters use
existing credentials and supported native resume; readable history is a separate
guarantee from native conversation recovery.
Reconnect recovers the result of the same launch request after a lost response.
An uncertain launch after supervisor failure requires reconciliation of any
surviving child before another launch in that worktree; see
[the launch decision](adr/0005-reconcile-uncertain-session-launches.md).

**Persistence (planned).** Keep repository working files separate from product
state. Store versioned configuration and durable records in the user's macOS
application-support directory, keyed by stable project/task/session IDs rather
than process IDs alone. A single supervisor-side persistence worker will own
SQLite transactions for task/session metadata, local review drafts, comment
choices, examined/validated/submitted revisions, and uncertain remote outcomes.
Bounded, append-only transcript chunks live alongside it; records retain their
paths after worktree removal. Do not save credentials or silently delete history.
Schema migration and crash recovery must be tested before persistence ships.
Saved edits require a durable acknowledgement. Streaming history uses bounded
buffers and batched persistence, targeting durable flushes at least once per
second under healthy storage. This is an operating target, not a maximum loss
guarantee during storage failure. Clean shutdown flushes pending history and
reports failed writes.
Low storage prompts explicit cleanup, suggesting oldest stopped sessions first;
history is retained until explicitly deleted. Storage failures remain visible,
and full buffers apply backpressure rather than silently drop output, while the
dashboard remains responsive. Backpressure on PTY reads is not a guarantee that
agent work or background commands have stopped. See [the history and cleanup
decision](adr/0004-preserve-history-with-explicit-cleanup.md).
The foundation writes no product state; selection lasts only for this run.

## Execution-machine boundary (direction; local execution today)

The first release runs locally on macOS. Keeping the option of a later VPS
execution machine means preserving ownership boundaries during local feature
work; it does not make remote execution a release requirement. This section is
the authoritative guidance for that direction. Existing ADRs retain their
specific ownership, evidence, durability and reconciliation decisions. A new ADR
is unnecessary while no remote deployment or protocol choice is being made.

### Boundaries to preserve as features grow

- **Supervisor interface:** machine-specific execution belongs behind the
  supervisor's request/observation interface. The supervisor and its workers own
  agent startup, PTYs, native-tool adapters, execution reservations and cleanup.
  The dashboard owns interaction and renders observations; it requests effects
  without reaching into the execution machine's processes or filesystem.
- **Checkout ownership:** repository discovery, canonicalization, Git commands,
  worktree mutation and checkout-file access belong on the machine holding the
  checkout. A returned checkout path describes that machine's filesystem; it is
  not evidence that the dashboard can open it. Preserve original path bytes
  within that owner and keep display labels separate from filesystem operands.
- **Identity and effects:** use explicit requests and observations with stable
  project/task/session identities as those models arrive. A path, PID, row index
  or connection is not a durable identity. Identify recoverable mutations by
  request identity and their target, so a reply can be attributed to the intended
  operation rather than whichever row is selected when it arrives. Keep bounded
  dispatch, saturation errors and durable completion visible to App.
- **Uncertainty and recovery:** connection loss makes current execution unknown;
  retain the last observation as historical context rather than infer stop or
  success. Recover a lost reply by querying the original request's outcome before
  retrying a mutation. Unavailable outcomes remain uncertain until reconciled.
  Launch already follows [ADR 0005](adr/0005-reconcile-uncertain-session-launches.md);
  later mutations need their own completion/recovery semantics. This is not a
  claim of exactly-once execution. GitHub publication still requires the explicit
  human preview/submission boundary and reconciliation of uncertain remote writes.
- **Editor artifacts:** diff generation belongs with the checkout or pinned
  review context. Return diff content and comparison identity (Working, WholeTask
  or Review, including revisions where applicable), allowing the dashboard to
  materialize an artifact for its configured editor without a local checkout.
  An execution-machine temporary filename alone is not a consumable artifact.
  Local editor scratch files and terminal handoff belong with the dashboard;
  edited review text returns through an explicit draft-save operation. This
  direction covers inspection/comment editing, not remote source-file editing.
- **Host policy:** keep supervisor acquisition/startup and endpoint selection in
  the client/bootstrap boundary, storage-directory selection in startup, durable
  state I/O in the supervisor's persistence worker, and OS process evidence and
  signaling in execution-side helpers. Native tools and `gh` use authentication
  on the machine running them; the dashboard must not assume its own login or
  credential paths are available there. Keep dashboard terminal/editor handling
  and macOS notifications with the dashboard. Retain existing native credentials
  rather than adding credential storage or copying secrets between machines.

### Current local assumptions and gaps

The session seam already has explicit `Request`/`Observation` messages and durable
task/session/launch-request IDs. The supervisor owns PTYs, launch deduplication,
history and process recovery. App retains attribution on connection loss, revokes
semantic confidence and exposes unknown process state. Recovery currently means
detaching locally and reopening, not automatic reconnection. Launch outcome
recovery does not provide a general outcome protocol for all commands.

The implementation still assumes a shared local host in these places:

- `main` passes the CLI repository path as bytes through `Request::Discover`;
  the supervisor owns discovery and canonical adoption workers. App matches the
  request and repository before accepting a discovery result and retains selection
  by `Worktree.path`. Registered durable project/workspace identity remains future
  work. The dashboard checks `git_jobs_protocol` in the owner's greeting before
  sending requests, and refuses incompatible live owners without migrating their
  state or sending unsupported commands. A second dashboard refused control by a
  compatible owner can still discover/refresh through the restricted
  `--discover-worktrees` execution helper. That bounded helper has no state
  directory or session/mutation commands; every session action remains refused,
  and quitting that dashboard exits locally without stopping the active owner.
- `session::transport` validates a local private state directory, starts the same
  executable with `setsid`, and connects to its Unix socket. Election and the
  single-controller policy are local-user policies under
  [ADR 0001](adr/0001-user-wide-supervisor.md), not multi-host ownership rules.
- `main` chooses `$HOME/Library/Application Support/rat/proof` or an explicit
  `--state-dir`. SQLite, transcript files, launch gates and native hook sockets
  use the execution host's local filesystem. Native tools inherit supervisor
  PATH/authentication; recovery and child cleanup rely on host boot/start identity
  and OS-specific process evidence. Linux fixtures do not qualify VPS support.
- The current dashboard freezes an owner UUID and expected managed session UUID
  in each coding control request. Selection is separate; late input, resize,
  Stop or reconciliation cannot target a replacement conversation. Legacy wire
  variants retain their selected-owner adapter. History supports explicit session
  UUIDs, with legacy index navigation retained. GitHub workflows remain planned. Comparison snapshots can carry bounded full-editor
  content and identity; the dashboard materializes inspection artifacts without
  reading checkout files. Source editing is explicitly local and requires an
  owner-validated existing source location.

Apply these boundaries when changing their owners, documenting any remaining
local assumption. Continue with concrete local modules and existing workers;
defer remote transport, provider abstractions, containers and extra crates until
remote execution is explicitly scoped and its deployment/recovery trade-offs can
be decided with evidence.

## Milestone 2 migration boundaries

Milestone 1 began with one global runtime/current adoption and singleton capture
accounting. Tickets 03, 05 and 06 now provide execution-owned asynchronous Git
identity, retained workspace/session records, targeted coding controls and fair
bounded per-session capture/durability accounting behind one writer. Each live
stream has its own queue, transcript handle, accepted/written/durable counters,
backpressure and error. Each rotating capture turn drains at most 64 blocks or
64 KiB, stopping after a completed write reaches a 2 ms budget; a failed write
ends that stream's turn. This amortizes derivative-index checkpoint commits
without enlarging capture queues. A blocking write can exceed the elapsed budget;
control remains supervisor-owned outside this worker. Rotating streams does not
force a commit. Final flush
waits only for its owned EOF's accepted prefix; another producer neither permits
nor delays that completion. Retained search now shares these finite turns with capture. Schema 7 stores
versioned full projection checkpoints and ordinal passages alongside an exact-word
posting index. One worker pass indexes at most 512 committed raw bytes for one
round-robin retained session, including at most one bounded 2048-cell unfinished
line; checkpoint, emitted passages and postings commit atomically. Indexing follows
capture and due flushes and never delays raw durable acknowledgement until an
entire transcript is indexed. A changed projection version prunes old passages in
bounded slices before rebuilding from committed raw; startup does not replay all
history synchronously.

One supervisor-owned browse/query identity is independent of runtime selection.
Queries inspect at most 32 keyset candidates per turn and return at most 32 rows,
with a fixed highest passage/record rowid per request. Exact-word lookups seek
postings instead of scanning raw prefixes. Opening a hit validates stable
session/version/ordinal/raw-range identity and reads that persisted passage, not a
raw page guessed from its beginning. Search editing, mode changes and close cancel
older work; late replies cannot replace the current query. Metadata Find and
retained-session browsing likewise yield between 32-row slices. Tombstones hide
text immediately, cancel pending searches and invalidate displayed/cache/editor
history; derivative cleanup removes one bounded passage per retained-row turn.
Native identities and launch outcomes remain independent of transcript deletion.

`workspace_history_facts(workspace,archived)` is the minimal retained archive-state
contract used by archive visibility. Schema 10 adds attributed archive operation
records independently of transcript deletion.
`checkout_branch_facts(path,branch)` contains only actual Git discovery facts and
is presented as last observed. Unknown legacy facts and missing/inaccessible
checkout paths remain distinct. See [the search evidence](proofs/m2-11-retained-search.md). Mutation/fetch work remains execution-owned and bounded. See
the [Milestone 1 implementation audit](../.scratch/milestone-2-coding-coordination/interview.md#milestone-1-implementation-audit-prerequisites-within-milestone-2).

The Milestone 2 interview retains one active coding session per worktree and
external source editing, while explicitly deferring multiple simultaneous native
sessions in one workspace and editing directly inside dashboard diffs. Preserve
these future paths through concrete data ownership, not a speculative framework:

- Workspace identity and session identity are separate. Workspace records own a
  collection of historical/current session identities; runtime, input, viewport,
  status and capture operations target a specific session. Enforce the current
  one-active-coding-session limit through reservations on canonical worktree
  identity, attributed to workspace/session IDs. Aliased or duplicate workspace
  records must not bypass checkout exclusivity. Keep this admission policy
  separate from the runtime collection and durable session schema.
  A later concurrency change still requires real shared-file/resize/input and
  native compatibility decisions; it is not merely increasing a numeric limit.
- Deepening launch and history completion must remain per Agent session, not
  create a new singleton runtime per Workspace. PTY, native conversation, status,
  input, cancellation and capture belong to that session; the Worktree and its
  branch are shared Workspace resources. Stopping or sealing one session alone
  cannot authorize archive, checkpoint, detach-for-branch-deletion or folder
  removal. Those Workspace operations require all relevant managed sessions and
  uncertain execution reservations to be accounted for and their capture tails
  complete, then hold off new admission while mutation is pending. This is useful
  with today's limit too; it does not implement future concurrent agents or
  promise control over arbitrary externally started processes.
- Keep named diff content, comparison identity and file/line attribution separate
  from Ratatui presentation and external-editor handoff. Future inline editing
  can use an explicit execution-side save operation with freshness checks; the
  renderer must not become an owner of checkout writes. Do not add placeholder
  edit controls, a plugin framework or unused backend abstractions now.
- Archive saves a local checkpoint when needed and pins the saved HEAD with a
  workspace-specific Git reference before optional local-branch deletion. A
  metadata commit hash without a live Git reference is insufficient protection
  from Git garbage collection. Retain the folder and ignored files; detach the
  stopped archived checkout at its saved commit when necessary to release its
  branch. Never force-delete a branch checked out elsewhere or overwrite a
  changed ref. Recovery must reconcile Git and durable metadata after a lost
  reply/crash, keep save/cleanup outcomes distinguishable, and recreate a branch
  on restore without overwriting a reused name. Archive refs and saved code are
  distinct from transcript deletion; no age-triggered cleanup is implied.

### Compatibility with later milestones

Milestone 2 changes concrete coding-session mechanisms. Preserve these boundaries
for Milestones 3–5 without implementing their features prematurely:

- **Review execution:** future restricted preparation can inspect a PR without
  a registered local project or coding workspace. Its review-cycle/comparison
  identity and revision-pinned snapshot remain separate from ordinary coding
  launch, arbitrary project setup and checkout mutation under ADR 0002. Reusing
  process/storage mechanisms must not grant those coding capabilities. Coding
  admission, one-at-a-time review preparation and sole SQLite-writer ownership
  are independent limits; the later review queue cannot serialize all coding.
- **Diffs, history and drafts:** coding comparisons and searchable terminal text
  are not substitutes for pinned review head/base identity (ADR 0006), structured
  draft comments/anchors/choices or exact saved editor text. Comparison artifacts
  identify their context. External source editing and future inline source edits
  are separate from Milestone 3 review-comment editing. History projections are
  derived data; later structured draft writes need their own durable records and
  fair storage scheduling, not reconstruction from transcripts.
- **Archive and own-PR continuation:** deleting a visible local branch must not
  erase retained workspace identity, original branch/base/checkpoint information
  or later linked PR/review records, nor imply merge, push or remote deletion.
  Milestone 4 PR links must use repository/PR identity independently of a reused
  or recreated branch name. A workspace archive ref is not a review comparison.
  Report checkpoint save, archive visibility and optional branch-cleanup outcomes
  distinctly, preserving confirmed recovery refs through partial failure.
- **Attention and validation:** historical or unacknowledged attention context
  remains separate from current evidenced semantic state under ADR 0003. Expired
  confidence stays Unknown; an old event must not masquerade as a current request.
  In-app coding coordination belongs here; macOS notification delivery/permissions
  and combined six-coding-session/PR/reviewer validation remain Milestone 5 work.
- **Launch/setup boundaries:** naming is metadata work, not permission to create
  a coding turn. Create without starting, startup, restore and reconciliation
  never launch an agent or submit a prompt. Any initial prompt belongs to an
  explicit Create and start request. Configured setup runs with owned process
  progress, cancellation, quit and uncertain-outcome handling; an uncertain setup
  command must not be replayed automatically because it can have external effects.
  This does not add review automation or general onboarding to Milestone 2.

The implementation sections below describe the completed Milestone 2 feature
boundaries. These design constraints do not establish the combined native,
resource or recovery acceptance gates; ticket 19 records that qualification.

## Native-session interaction baseline (planned)

This is the design baseline for Milestone 1, not a description of implemented
session support. Build it with the PTY/supervisor proof rather than performing
a behavior-preserving foundation refactor first.

**Deepen `app`, rather than add an interaction module.** Its interface accepts
terminal input, worker observations, and session viewport changes, updates local
interaction state, and returns explicit effect requests. It performs no blocking
work. Focus-sensitive interpretation belongs here, including paste and native
shortcuts; a global key-to-action mapping must not discard attached input before
focus is considered. Keep `events` as the vocabulary for inputs, observations,
and effects, rather than a second owner of interaction decisions. Internal
helpers need no adapter traits.

`App` owns selection, input focus, pending user requests, local errors, and the
latest observed session state. The supervisor owns adoption records, stable
identities, worktree exclusivity, launch outcomes, process lifecycle, and history.
A launch effect carries a stable request identity; reconnect queries that same
outcome. Dashboard state never decides that a disconnected or quiet session has
stopped, that an uncertain launch is safe to repeat, or that metadata is saved
before durable acknowledgement. This preserves the ownership and recovery
decisions in ADRs 0001 and 0005.

**Keep `main` mechanical.** It polls input, delivers worker observations, dispatches
effects without waiting, and renders. Workers return typed success, failure,
and connection observations to the same application interface. Effects include
Git refresh, session input/resize, launch/attach, stop, detach, and shutdown;
only expose actions once their execution works. Bounded dispatch must report
queue saturation or failure rather than silently discard input. Bound event
processing by payload and time as well as count; never enqueue an entire growing
transcript as one event. Busy output cannot starve input, stop, or recovery.

**Make exit a state transition with an outcome.** Returning to dashboard focus
changes only input ownership. Explicit detach closes the dashboard connection
and preserves execution. Ordinary quit enters pending shutdown and exits only
after confirmed stop and history flush. A failed or uncertain shutdown remains
visible and retryable; it is not a successful quit. Unexpected client loss does
not request shutdown. In attached focus, Ctrl-C goes to the native session;
dashboard quit bindings do not intercept native input. Choose the reserved
escape chord through the native proof, not through an untested key convention.

**Keep geometry and rendering in `ui`.** Calculate the session's usable interior
rectangle once for both drawing and resize requests. `App` receives dimensions,
not Ratatui layout rules. Rendering uses bounded screen state, does not execute
effects, and shows available actions from the same interaction policy that
handles input. Tiny or absent session viewports must not send invalid PTY sizes;
retain the last usable size until a usable viewport returns. The supervisor
serializes PTY/parser resize and supplies coherent screen synchronization.

**The interface is the test surface.** Exercise focus, pending requests, shutdown
outcomes, and reconnect observations through the application interface used by
the dashboard. Use rendering tests for help, focus, and viewport behavior. Keep
the approved real-dashboard/supervisor/scripted-PTY seam for actual input bytes,
sizes, child ownership, durable history, and failure responsiveness. Native-tool
compatibility is qualified only by recorded macOS evidence. This gives locality to
interaction policy and leverage to tests without a generic worker framework or
mock adapter for each module.

## Investigate the interactive-session boundary first

Ordinary piped stdin/stdout is insufficient for native interactive agents. Spike
a PTY library (for example `portable-pty`) plus a terminal screen parser on
macOS. The supervisor continuously drains PTY bytes, even with no attached UI;
keep a bounded screen/scrollback buffer and spool durable history outside the
render/input path. Slow disk or subscribers need bounded queues and explicit
backpressure/failure reporting rather than unbounded output accumulation. PTY
spawn, read/write, resize, and process reaping all happen off the UI loop.

Only the dashboard controls the real terminal. An attached-session view renders
the emulated child screen; it does not print raw escape sequences over Ratatui.
Dashboard focus handles navigation; attached focus forwards terminal input to the
PTY, including native Ctrl-C. Reserve and document an escape chord to return to
dashboard focus without killing the child. Validate encoding, paste, prompts,
alternate-screen programs, mouse modes, and partial escape/UTF-8 sequences rather
than assuming key events reproduce every native shortcut.

Resize events must update both PTY dimensions and screen-parser dimensions in
order. Detached sessions continue draining output; attaching synchronizes a
screen snapshot plus subsequent output without gaps or duplication. Verify
session reconnect, startup errors, native exit and resume, bounded memory under
sustained output, and process-group cleanup. Record supported tool versions and
real macOS results. Linux fixtures alone cannot prove either native integration.

## Review and validation boundaries

Later GitHub workers use existing `gh` authentication and a serial preparation
queue. The background reviewer receives revision-pinned context under enforced
restrictions against arbitrary shell execution, task-checkout mutation, and
GitHub writes; see [the preparation decision](adr/0002-restricted-review-preparation.md).
Both native integrations must validate this contract with existing authentication
before preparation ships. Draft preparation, local editing, submission, and
merge commands remain distinct effects. Local drafts identify the examined
revision and any later human-validated revision, retaining both head and
base/comparison revisions used for the diff; see
[the comparison decision](adr/0006-identify-both-sides-of-review-comparisons.md).
Publishing requires an explicit user action on an
exact preview and a fresh revision/eligibility check. A revision change blocks
submission until revalidation, including a changed base/comparison; uncertain
outcomes require reconciliation before
retrying. Background failures preserve human edits and never label partial work
ready. No GitHub access is part of the foundation.

Verify boundaries with temporary real Git repositories, deterministic state/event
tests, and terminal smoke checks. Next add a scripted PTY agent, supervisor
failure tests, and measured navigation/attach latency, then real Claude Code,
Codex CLI, editor, and recovery checks on macOS. CI formatting, Clippy, builds,
and tests support those checks; they do not substitute for native evidence.
Implementation must also define and measure memory and queue limits, transcript
disk growth, persistence throughput, and slow/full-disk behavior under sustained
output. Include detached sessions and slow subscribers; report the workload and
resource measurements alongside responsiveness results.

## Scripted PTY prototype (ticket 01)

This section records ticket 01; the subsequent ticket sections describe the
current extensions to launch, rendering, lifecycle and recovery.

`session` now contains the fixture runtime, supervisor mode, private Unix-socket
protocol, and dashboard transport worker. `App::input` owns focus and returns
explicit refresh/session effects. The event loop dispatches without waiting;
Git and session workers deliver observations. `App::observe_session` applies
coherent screen snapshots. The UI places unavailable reviews above the project/
worktree tree, beside a native-screen pane and visible focus/help.

The supervisor launches only the same executable's scripted-child mode, directly
in the adopted canonical checkout. Arguments and original Unix path bytes cross
the socket without shell interpolation. Task/session UUIDs originate in the
supervisor and the launch-request UUID originates in App; all are in-memory only.
`portable-pty` 0.9 provides PTYs and `vt100` 0.18 parses incremental output into a
fixed 100×24 screen with zero scrollback. This proof displays plain parsed cell
text; terminal cell styling and dynamic viewport synchronization remain future work.
Commands (32), UI snapshots (1), socket frames (64 KiB), PTY read chunks (4 KiB),
and input chunks (4 KiB) have finite caps. Slow UI snapshots resynchronize on the
next poll. Blocking socket, filesystem, child, and PTY work runs outside the UI
loop. There is no transcript capture or durability claim in this slice.

This is a thin first slice, not completion of the planned interaction baseline.
Launch reconciliation,
persistence, lifecycle evidence, native adapters, geometry,
remain follow-on work. Failed shutdown remains visible and
retryable; no confirmed stop means no successful quit. See
[the prototype evidence](proofs/01-scripted-pty.md).

## Session ownership (ticket 02)

The default application-support directory identifies the user-wide supervisor.
Each startup opens the same permanent, user-private lock file and tries an
exclusive OS lock. A losing contender exits; the winner retains the lock for its
whole lifetime. Under that lock it removes an old socket only when connecting is
refused. A live endpoint without the lock, an unexpected endpoint type, or an
uncertain connection error fails closed. The state directory must be a real
user-owned directory with no group/other permissions; lock and socket are private.
The lock file is never unlinked, avoiding replacement-inode election races.

A startup handshake grants or refuses control before commands. The listener
continues accepting while a controller is connected, immediately refusing other
clients without dispatching their commands. Bounded nonblocking framing detects
EOF/socket failure and releases control while preserving the child and parsed
screen. Supervisor startup creates a separate OS session so losing the dashboard's
controlling terminal does not send it SIGHUP. Worker-only socket handling keeps
ownership errors outside rendering and preserves successful Git discovery.

Adoption resolves Git's working-tree root and canonicalizes it, unifying symlinks
and subdirectory paths without merging distinct linked worktrees. Repeated launch
retains the existing identity and refuses execution; same-checkout adoption and
attach retain the child. This slice remains limited to one prototype child total,
with no persistence or supervisor-crash survivor reconciliation. See
[recorded ownership scenarios](proofs/02-session-ownership.md).

## Input and focus proof (ticket 03)

`App` interprets keys only after focus: native keys become bounded key-event
requests; dashboard shortcuts apply only in dashboard focus. Ctrl-] and its
legacy Ctrl-5 alias return focus without a stop request. Focus-dependent help is
provided by the same application policy. Key encoding, paste encoding and delimiter
selection run in the supervisor, using the parser's current input modes. The
transport's bounded command queue reports saturation; paste is limited to 4084
UTF-8 bytes before wrapping, and ordinary input remains limited to 4096 bytes.
The parser supplies cursor-key mode at encoding time. Only the
dashboard enables the outer terminal's bracketed-paste mode; its terminal guard
restores raw mode, alternate screen, cursor visibility and bracketed paste after
normal return, errors and panic. See [recorded evidence](proofs/03-native-input.md).


## Fixture lifecycle (ticket 06)

Process lifecycle is explicit (`Idle`, `Running`, `Stopping`, `Exited`, `Stopped`,
`StopFailed`) and separate from transient semantic agent status and task archival.
The supervisor polls child exit even without a controller; native exit retains
in-memory attribution and the bounded screen. Stop keeps the dashboard open;
quit enters App pending shutdown and exits only after a confirmed observation.
A stop failure preserves the child reservation, shows an actionable error and
allows explicit retry. Final shutdown observations use reliable event delivery
rather than the coalescible screen-only path. Detach exits through the normal
terminal guard without sending Shutdown. Client EOF merely releases control.

A termination worker owns the child while stopping. The live supervisor captures
the gated helper's UID/start identity before authorization and tracks verified
descendants across process groups on a worker (100 ms sampling, 1024 retained
identities). Before termination it freezes the leader and each verified child
generation, requires stopped-state evidence and a second inventory adding no new
members. TERM remains pending while frozen; KILL follows after 500 ms, without
reopening a fork/reparent gap. Stop is forceful: native shutdown hooks and native
conversation buffers need not flush. Native `/exit` is the separate graceful
path; managed captured history still needs EOF and durable tail completion.

Inventories cap at 8192 processes, 1 MiB and 500 ms; closure has a 500 ms budget
within the two-second lifecycle deadline. Every signal checks UID and OS start
identity. Inspection errors and exceeded bounds retain StopFailed/authority for
retry; stopped records drop tracking. Restored records never reconstruct signal
authority. Zombies cannot execute. Unrelated daemons outside the live ancestry
are excluded. Deliberate double-fork/reparent escape between samples and the OS
inspection/signal interval remain limits; no kernel sandbox is claimed. See
[the actual native failure and correction](proofs/18-native-child-stop.md).
No launch or adoption replacement is allowed while termination is pending or
unconfirmed. PTY input uses a separate worker with 32 bounded 4096-byte slots;
queue saturation reports retry, and a blocked writer cannot block stop commands.
No persistence or clean-shutdown durable flush is claimed before ticket 08.
See [lifecycle evidence](proofs/06-lifecycle.md).
### Scripted screen proof (ticket 04)

The current prototype derives child geometry from the shared session-pane layout
(sidebar, status/help and borders excluded), capped at 200 columns × 80 rows.
An absent viewport retains the last usable PTY/parser geometry until it returns.
The supervisor serializes PTY resize then parser resize against 4 KiB output
processing; the reader continues with dashboard focus or no connected client.
There is no scrollback or durable transcript yet. Full styled screen snapshots
carry the parsed-output byte boundary, geometry and cursor/input modes. The
transport worker reconstructs display cells; rendering reads those cells and
never writes raw child controls to the outer terminal. One queued snapshot
resynchronizes lagging dashboards; this is ephemeral screen state rather than
retained history. Framed transport is capped at 4 MiB for bounded styled snapshots.
See [the proof and visual comparison](proofs/04-session-screen.md).

### Experimental Claude launch (ticket 05)

Dashboard `c` explicitly starts `claude` from the supervisor's inherited PATH,
with the PTY working directory set directly to the adopted canonical checkout.
`l` remains the safe scripted fixture choice. Native credentials and prompts stay
with Claude; the product has no credential store and makes no global/repository
configuration edits. Startup errors explain installation/login and explicit retry,
while discovery and adoption remain available. A live child refuses either launch
choice. The observation labels the launched tool independently of process lifecycle;
semantic status remains unknown. See [native proof](proofs/05-claude-native.md)
for versioned results and support limits.


## Dashboard recovery (ticket 07)

The supervisor issues a launch-request identity before any launch and includes it
in greetings and observations. App restores it after reopening. Every accepted
launch request retains its original success or failure in a lifetime result map;
`QueryLaunch` and same-ID launch retries recover that result without executing.
An explicit launch after an acknowledged outcome creates a fresh request. The map
has 1024 entries, request IDs have a 128-byte bound, and capacity exhaustion fails
closed without evicting outcomes. These are owner-lifetime records pending ticket
08 durability; they make no cross-supervisor execution guarantee. Unknown request
queries explain uncertainty rather than automatically replaying a launch.

Control-command observations use reliable bounded delivery; only periodic screen
snapshots may coalesce. Reconnection synchronizes ticket 04's bounded styled screen
and output-byte boundary, then resumes polling while the same child keeps draining.
Client death releases the controlling connection without stopping children. A live
controller continues to exclude every competing dashboard. See the
[reconnect proof](proofs/07-dashboard-reconnect.md). Supervisor failure, uncertain
survivors and launch reconciliation belong to ticket 09.

### Experimental Codex native launch (ticket 11)

Dashboard `x` sends the same request-identity launch contract as Claude `c` and
fixture `l`. The supervisor executes `codex --no-daemon` directly in the adopted
checkout using its inherited PATH and existing native authentication. Keeping
Codex out of its shared daemon preserves managed process ownership; no shell
interpolation or credential copy is involved. All launch choices share active
worktree exclusion and replay their known request outcome. Changing tool requires
stopping the active session and starts a new native conversation. Native setup
and authentication remain in the CLI; help explains `codex login` and failed
spawn preserves adoption. See [recorded native proof](proofs/11-codex-native.md)
for version-specific observations. Semantic status remains separate/unknown until
the Codex status ticket establishes evidence.

### Durable readable history (ticket 08 implementation)

A single `persistence` worker owns the supervisor's SQLite connection and raw
transcript files. Its versioned schema migrates transactionally from empty state
through version 1 (app/session metadata), version 2 (launch outcomes), and
version 3 (nullable creation age, final-capture seal and history tombstones), and
version 4 (projects, workspaces, opaque task aliases and session membership).
`registry` owns canonical matching and relational persistence behind the existing
sole storage worker. Projects use byte-exact canonical common directories;
workspaces distinguish canonical Git directories within them. Names, selected
rows, branches and folder availability are not identity. Existing task/request
strings remain opaque. Migration groups only explicit task facts, retains unknown
legacy paths/projects, checks durable UUID consistency, and rolls back visibly on
unattributable/conflicting records. It does not change transcript extents, seals,
tombstones, native IDs or request outcomes. The current app path bytes are retained
only for their explicit task, never assigned to unrelated historical sessions.

The supervisor separately owns registry snapshots/registration operations and its
collection of session owners. Only explicit adoption or runtime selection changes
the durable selected workspace; metadata saves from another running owner cannot
replace it. Selection keeps metadata pending until its exact private request UUID
receives a committed selection result; an unrelated registry snapshot cannot settle
that request. Launch-outcome lookup spans all owners, while replay still respects
the selected owner's provisional launch reservation and never changes its control
target or native session identity. Startup retains the latest session per canonical workspace plus every
unresolved execution (at most 1024 saved session records, failing closed beyond
that bound), without launching tools or allocating capture queues for history.
Up to 32 unsettled execution/capture owners are admitted. Each stream retains
64 queued blocks of at most 4096 bytes, one worker block and one producer block
(270336 accepted bytes before the reader's separate finite 1 MiB Stop tail).
The control queue stays independently bounded at 16 commands. Successful sealing
releases that stream's live queue and handles. The owner retains its finalized
byte counters and bounded commit evidence through the seal receipt and reconnect;
an unfinished tail remains reserved.
Accepted, written and durable counters describe the current managed-session stream,
reset for its replacement and restore only its committed prefix. The separate
`output_bytes` counter remains cumulative for that in-process owner's screen
across conversations and restarts at zero on owner restoration; only it requires
a launch baseline. Earlier M1 measurements retain their historical interpretation.
Actual write/fsync syscall stalls still stall the sole writer for all streams.
Native launch gates and Claude/Codex hook endpoints use distinct private short
`/tmp/rat-<uid>-<nonce>` directories so simultaneous launches cannot replace one
another's endpoints and macOS's 104-byte Unix socket pathname bound is respected.
Their runtime directories are removed on owner drop. Explicit registration uses its bounded Git
verification worker and an attributed storage command; only its matching committed
completion publishes rows. Registration does not retarget the active session.
Adoption selects an existing canonical workspace's retained task mapping. Registry
reads and legacy migration never launch/resume native tools. Dashboard `w` opens a
stable-ID picker (including compact terminals); `A` registers, Enter opens an
existing runtime (or discovers a workspace without one), `a` explicitly adopts, and `h` reads the selected workspace's latest session by UUID.
Discovery remains a separate context, so registration/adoption cannot mislabel
previous worktree results as a different repository. An independent registry
protocol capability is checked before sending any command to a live supervisor;
lock election still precedes opening/migrating storage. Unsupported live owners
remain authoritative and get compatible-dashboard stop/upgrade guidance.

Records
retain task/session/request identity and selected history position; checkout path
bytes are retained for the current adopted task independently of checkout existence.
The supervisor restores active records as interrupted and never starts a child
from restored records. Replacement launch stays blocked until explicit supported execution reconciliation
(ticket 09 below). Native launch phase/identity crash-window hardening now uses
a durable launch gate; completed metadata operations are not claims of exactly-once
process execution.

Capture transfers each at-most-4096-byte PTY read into a 64-slot lossless queue
before applying it to the screen. The reader retains its current block while
waiting and takes no screen lock during storage backpressure. The sole worker
retains its current block on append failure. Fixed 65536-byte files rotate by
stable UUID plus ordinal, independently of checkout paths. A 16-slot control
queue and 16-slot completion queue are separate from append traffic. Metadata
messages omit native screen buffers. Raw history pages cover at most 2048 bytes
and remain escaped before UI delivery. A stalled disk does not run on the dashboard/supervisor loop;
shutdown remains pending while the sole worker is unavailable. This is bounded
backpressure, not a claim that an agent stops all non-output work.

The 250 ms flush timer synchronizes the transcript file with `File::sync_all`,
then commits its byte extent in SQLite with `synchronous=FULL`, DELETE journaling,
and `fullfsync=ON`. Newly created chunk entries are directory-synchronized before
extent acknowledgement. Success/progress appears only after both operations.
A stopped runtime drains to PTY EOF before the final flush; quit additionally
waits for lifecycle metadata completion. Read/flush errors remain visible.
Recovered history reads only committed extents; a surviving uncommitted tail is
not counted or overwritten. Raw keys and paste are not copied into state.
The raw view retains escaped terminal operations and redrawn/alternate-screen
content. The default readable derivative uses `history::Projection`, independent
of native records, paths and naming. The incremental UTF-8/control parser keeps
bounded line state (2048 scalar positions), at most four UTF-8 bytes and 32 CSI
parameter bytes; control-string bodies are never retained or executed. Horizontal
cursor/CR/backspace/erase edits affect the current line; vertical redraws become
separate passages, so duplicates, fragments and geometry artifacts remain possible.
The last page line is a provisional preview, explicitly marked in passage metadata.

A readable request first receives the escaped raw page with a preparing flag.
Repeated polls for the same owner/session/passage share that request through its
final readable result or error. A different passage invalidates the old request.
After completion, a same-passage refresh keeps the cached page visible while
reading its newer committed prefix; a matching failure clears any preparing page
and permits an explicit retry.
The sole worker then replays at most 32 KiB per turn, interleaving capture, control,
cleanup and flush; completion retains owner and request UUIDs so late results and
errors cannot replace another selection. At most 16 jobs and 64 parser checkpoints
are kept across all sessions; checkpoints are derivative, process-local and rebuild
on restart. Cold distant reads can require many bounded turns. Checkpoint keys use
session/raw position in the current projection version; only immutable committed
prefixes are eligible. Every turn rechecks retention/committed extent and deletion
intent evicts matching checkpoints. A readable result carries projection version,
source raw range and committed extent; passages map UTF-8 text byte ranges to raw
ranges under the same stable session UUID. Search/index scheduling remains ticket 11.
Reading requires neither the checkout nor the original native conversation. Power-loss, full-volume and sustained resource
capacity proof remain separate gates; see `docs/proofs/08-durable-history.md`.

### Durable launch authorization and explicit reconciliation (ticket 09)

All three launch choices now use `recovery`'s temporary same-executable gate.
The SQLite worker first commits prepared task/session/request identity and boot
identity before any process is created. A helper connects to a private nonce
socket and cannot execute coding until the supervisor receives the worker's
separate durable authorization completion. That authorization records PID,
process start time and boot identity before the release byte is sent. The helper
then execs the selected native tool (Codex retains `--no-daemon`); it becomes that
process rather than remaining as another session daemon. Native exec failures
return a distinct receipt and are saved as failed launch outcomes. A successful
close-on-exec receipt is checked against the live process identity before a
successful outcome is committed and acknowledged. This is not exactly-once
execution: crashes after durable authorization can leave an uncertain outcome.

Restore never releases a gate or starts an agent. It marks retained execution
Interrupted and holds adoption/replacement exclusivity. `QueryLaunch` and retries
of the same ID recover the retained outcome, including interrupted/uncertain
results synthesized and committed when the crash preceded a known result.
The 1024-result limit stays fail-closed; this change does not evict requests or
load unbounded history. Prepared and authorization completions use the existing
bounded persistence control channel, independently of transcript bytes.

Dashboard `u` explicitly reconciles. macOS evidence uses `kern.bootsessionuuid`
and `proc_pidinfo(PROC_PIDTBSDINFO)`'s microsecond process start time. A matching
live identity blocks replacement and is never signaled by recovery. A different
verified boot proves previous processes cannot survive. A never-authorized gate
record also proves no coding could execute, so it can release its reservation.
Reconciliation preserves Interrupted and history; starting a new conversation
remains an explicit separate choice. `s`/`q` cannot claim confirmed stop of an
unmanaged uncertain survivor; `d` leaves the recovery dashboard.

A missing or reused PID does not establish descendant absence. After native
execution was authorized, leader absence on the same boot can require an actual
verified reboot before replacement. Process-group inventory provides an
explanation, not permission to ignore escaped descendants. Unavailable identity
or inventory remains uncertain. Legacy active/already-Interrupted records without boot evidence acquire a
separate durable first-recovery `observed_boot` anchor. Same-boot recovery stays
blocked; a later verified different boot can release them without inventing a
historical launch boot. A durable reconciliation marker preserves explicitly
reconciled Interrupted records across restart. The Linux fallback
uses `/proc` boot/start fields for fixtures; it establishes no native macOS
compatibility. See [the crash-window proof](proofs/09-launch-reconciliation.md)
for the exact boundaries, simulated boot evidence and limitations.

### Partial creation ownership (Milestone 2 ticket 02)

A single bounded creation worker immediately retains a successfully spawned
child, its PTY pair and unopened launch gate. The supervisor reserves that
in-flight attempt while servicing Screen, Stop and same-request queries. Stop
cancels release intent; a late creation result must return for verified cleanup
before shutdown or replacement can finish. Child and master ownership transfer
before fallible identity/input/capture setup. Failed scope capture retains the
actual child for an explicit Stop retry. Input and capture install before the
authorization command is queued; a setup error drops release authority and
retains its error. Native status polling begins after installation, including
execution whose durable result is still pending. The existing live-descendant
cleanup and durable launch/recovery machinery remain authoritative. See the
[fixture proof and native limitations](proofs/m2-02-partial-spawn-ownership.md).

### Claude semantic observations (ticket 10)

`native_status` owns a bounded passive invocation-local hook channel. Correlated
AskUserQuestion/Bash permission events plus the demonstrated 2.1.318 native modal
establish attention. Working requires submitted-turn evidence, a new native
response header, current active footer, fresh output and no pending tool interval.
It does not classify pre-output thinking or a blocked submission/tool as work.
Completion requires current-turn Stop, a later parsed boundary, a new native done
marker and the idle bordered composer after hook resolution. A one-shot Stop
continuation remains unknown until the final Stop/native idle structure agrees.
Interruption requires an active native screen at a real Ctrl-C request and a
newly observed native acknowledgment/idle composer within five seconds. Existing
quoted/native-looking acknowledgment text and question/approval cancellation do
not establish interruption. Conservative layout, marker and expiry checks can
leave genuine outcomes unknown. Missing/invalid/stale/wrong-invocation evidence
revokes confidence; status is persisted/restored as unknown. Native monitoring
stays separate from process ownership, resume identity and transcript capture.
See [the exact version/scenario proof](proofs/10-claude-status.md).

### Slow-subscriber isolation (ticket 13)

The supervisor serializes one finite response at a time. A retained partial-write
cursor advances through at most 64KiB of nonblocking socket output per pass;
control/ownership and storage completions continue between passes. A frame that
cannot complete in 500ms closes its socket and records a visible reconnect
reason. Reopening receives a coherent current replacement snapshot with its
parsed-byte boundary and the same session identity, never an output delta backlog.
The exact transcript is independently captured through 64 x 4096-byte slots,
with a worker block and a waiting producer block: at most 270336 pending owned
raw bytes during normal capture. On Stop, the same reader may additionally
retain a FIFO tail of at most 256 blocks (each <=4096 bytes, at most 1MiB raw),
with bounded per-block descriptors/session IDs. This lets macOS terminal exit
drain queued output while storage remains blocked. Tail exhaustion restores
backpressure; it never relaxes child reaping or live-group absence requirements.
The reader transfers the tail in order before EOF completion, so final flush,
replacement launch and history cleanup still wait for its durable capture.
Capture blocks on saturation and reports pending high-water and actual
backpressure count/wait time. Screens retain zero scrollback and at most 200x80
cells per primary/alternate screen. vt100 defaults are disabled so vte uses a
fixed 1024-byte OSC payload rather than a growable control-string Vec; the exact
raw control remains in durable history. Protocol frames remain capped at 4MiB.
Connection loss keeps the last identity/lifecycle, labels process state unknown
and revokes semantic status. `d` exits locally even after transport loss; reopen
resynchronizes without launching. `q` still requires confirmed stop and flush.
See [caps and preliminary resource proof](proofs/13-slow-subscribers.md).


### Codex semantic observations (ticket 12)

`codex_status` uses the shared semantic State/Status vocabulary through a bounded
passive invocation-local hook channel. A token, validated native session UUID,
submitted turn UUID and pending tool IDs bind receipts to the managed invocation.
PreModelRequest/PostModelRequest bound working evidence; correlated question or
permission intervals plus the current 0.167.0 native modal establish attention.
Native Interrupt acknowledges active-turn cancellation. Stop alone does not
prove completion: a later output boundary, a new native done marker and the idle
composer/footer must agree. New input/requests, expired or invalid receipts,
ambiguous attribution and changed modal screens revoke confidence to unknown.
No terminal prose, silence, intermediate command failure or live process invents
semantic outcomes. Every native state is independent of process lifecycle,
connection and task archival, and expires conservatively (five seconds for model
requests, 30 otherwise). Native hooks retain existing policy and require native
trust; monitoring failure does not prevent launch. The execution gate wraps the
fully configured native argv. Detached monitoring refreshes the parsed screen
only when its output boundary changes. See the [exact version/scenario proof](proofs/12-codex-status.md)
for five demonstrated signals, Stop-continuation and stale/delayed/quoted-text
negative checks, and unsupported layouts/version limits.


### Explicit native conversation recovery (ticket 16)

Observation metadata retains an optional native conversation UUID independently
of process state and bounded history chunks. Claude gets a generated UUID at new
launch; resumed monitoring uses the original ID without a fresh --session-id.
Codex supplies its own UUID through the invocation-local SessionStart monitor;
the supervisor validates/persists it through the sole storage worker. A resumed
Codex monitor expects that original identity. Live hook tokens and semantic state
remain transient; no credentials or native transcript payloads are copied into
this field.

The explicit Resume request uses the existing durable launch gate and request
cache. It requires the retained native tool/ID and a present checkout, creates a
new managed runtime/history segment under the same task, and invokes supported
native resume without a follow-up prompt. Startup and reconciliation never
invoke it. Uncertain execution and one-active-session restrictions apply before
native recovery. Unavailable recovery explains the reason; new conversation
choices remain explicit. Readable historical segments stay independent of native
records and checkout existence. The original latest-conversation evidence remains
in [ticket 16](proofs/16-native-recovery.md). Milestone 2 adds explicit historical
selection as described below.

`ResumeSelected` freezes a launch request, workspace UUID and source managed
segment UUID. These differ from the transient control-target UUID, original
native conversation UUID and newly allocated managed segment UUID. The sole
persistence worker validates workspace membership (including retained task
aliases), row/decoded segment consistency, original supported tool and native
UUID. It returns only an immutable resume descriptor; it never installs a
historical Observation in a live owner. Schema 9 binds request UUIDs to canonical
workspace/source choices independently of transcript rows. Changed-source reuse
and durably cancelled choices are refused after restart.

A resolving owner reserves its canonical checkout and reports the frozen request
for reconnect queries. Stop disarms lookup; a late descriptor cannot launch.
Cancellation retains a reservation until the sole writer acknowledges it. After
lookup, admission rechecks setup readiness, setup/canonical/uncertain owners and
capture settlement before durable Prepare. Prepare already identifies the chosen
tool/native conversation in the fresh managed segment. Prior monitors are retired
at the generation change, preventing a late previous-tool receipt from replacing
that identity. The existing Prepare/create/authorize/result gate and global
request cache then apply unchanged.

Conversation rows show workspace, original tool, segment and native IDs. Their
explicit `v` action routes to that workspace; an occupied workspace selects its
reserved owner and offers attachment. Reading/paging/search passage opening,
raw/readable toggles and restoration cannot change the source of pending resume.
The latest action resolves its frozen saved segment through the same worker.
Unavailable metadata or checkout explains the limitation and leaves new
conversation choices explicit. Native record/version rejection remains native
terminal output in the attempted new segment; process liveness never establishes
successful recovery. No Git mutation, prompt transfer or startup resume occurs.
See [selected native recovery evidence](proofs/m2-12-selected-native-resume.md).


### Slow and failing history storage (ticket 14)

The sole persistence worker supports explicit RetryStorage (`y`) through a
supervisor-side latch, independent of the 64-block capture queue and retained
even when the 16-slot storage command queue is full. No replacement writer is
started. A stalled syscall must return before that worker can retry; controls
and process-stop work continue separately. Append errors retain the exact block
and retry with a 100ms pause rather than spinning. Each attempt seeks the
unchanged logical offset, so a real partial write is overwritten identically.
Opening a chunk always retries its directory-sync obligation, including an
existing empty chunk left by a failed directory sync. Storage error clearing
requires successful append and the subsequent successful durability boundary.

Observations expose owner-lifetime accepted PTY bytes (including the blocked
producer block), current-session logical written/durable prefixes, pending/high
water bytes, operation age and ongoing producer-wait age. Relaunch comparisons
must baseline owner-lifetime counters. A written prefix is not a durable prefix.
Final flush progress is published before its completion event and shutdown ACK.
Stop can confirm process cleanup while history remains pending; replacement
launch/adoption cannot strand that prior capture tail. Stop during capture
trouble retains a possible-loss notice: accepted bytes are retained, while
unread native/kernel output may be lost when the process terminates. There is
no output discard or tcflush path, and backpressure does not suspend arbitrary
agent work or descendants.

Prepare/Authorize/result persistence now return honest provisional observations
(`metadata_pending`, no launch_result) so the existing half-duplex client can
send Stop, Shutdown, Screen and RetryStorage. The supervisor continues admitting
and servicing connections throughout these stages. Durable session commands and
success/failure completions carry operation UUID, session UUID when available,
request identity and purpose (preparation, authorization, launch result, metadata,
final flush or history sealing). The supervisor tracks each accepted operation;
a duplicate or altered completion cannot retire another operation. Preparation
and authorization require the current session/request, and launch acknowledgement
requires the exact result operation rather than a global pending-write count.
Stop cancels only the current owner's pending intent and drops its release sender. Late commits cannot start a canceled request.
Authorization must still commit before gate release, and only committed results
are acknowledged. Restored uncertain execution remains reserved and cannot be
cleared by Stop. Same-request durable outcomes and unknown-request recovery
remain in force. See `docs/proofs/14-storage-faults.md` for fault boundaries,
raw observations, kernel-output loss and the macOS exit-verification finding.

### Explicit retained-history cleanup (ticket 15)

The sole storage worker samples `statvfs` on the state filesystem once per second;
`f_bavail * f_frsize` uses a saturating u128 intermediate. Available bytes below
1 GiB warn, equality does not; errors report unknown capacity. This separate
capacity observation never clears a capture fault. `g` opens eight-row cleanup
pages ordered by final-capture eligibility, known creation time ascending, then
stable insertion order. New records capture their actual insertion timestamp;
schema migration leaves legacy timestamps NULL. Allocated transcript size uses
`fstatat(AT_SYMLINK_NOFOLLOW)` block counts, includes uncommitted chunks and is
unavailable when any matching object cannot be safely inspected. Directory work
processes at most 64 entries per worker pass, interleaved with capture/control.
SQLite/shared metadata bytes are not attributed to individual sessions.

The supervisor seals a segment only after its owned PTY has reached accepted EOF,
final transcript sync/commit succeeded, metadata committed, execution stopped and
capture queues drained. The worker checks saved stopped/exited state, closes its
retained transcript handle and commits the seal. Restored Stopped alone cannot
mint this proof. Legacy unsealed and uncertain/interrupted records stay visible
but unavailable for deletion. Worker and supervisor recheck eligibility at
confirmation. App stores a copy of the previewed UUID so reordered observations
cannot retarget confirmation. Enter previews; Shift-D confirms; Esc returns
without submitting deletion. Attached focus forwards native keys unchanged.

A FULL SQLite transaction commits `history_state=deleting` before any unlink.
Readable-history queries exclude deleting/deleted segments, append refuses their
UUIDs, and later metadata saves cannot resurrect them. Only canonical UUID plus
ordinal transcript names are considered. An O_DIRECTORY/O_NOFOLLOW directory
handle anchors `fstatat` and `unlinkat` to the validated private state inode;
symlinks, non-regular files and hard-linked chunks are refused. No checkout or
record-supplied arbitrary file path is used for deletion. The worker incrementally
unlinks selected chunks, synchronizes that directory handle, then commits
`history_state=deleted` and resets that segment's durable extent. It retains all
session/native-conversation metadata and durable launch outcomes. A failed intent
changes no files; later failures keep the durable incomplete tombstone. Explicit
retry or restart of an already-authorized intent safely scans surviving chunks;
there is no pressure/age-triggered policy and no VACUUM or native-record pruning.

Only after durable completion does the worker publish consistent current-segment
accounting, re-sample capacity, and notify the supervisor to clear cached history
and normalize selection. Other segments' counters/files remain intact. Cleanup
messages persist in the status area and cleanup pane. Low space can persist;
RetryStorage (`y`) and Stop (`s`) remain available. Cleanup success never clears
pending bytes, a write/sync fault or possible-loss context. Test faults are debug
files under the explicit isolated state directory, extending ticket 14 with
available-capacity injection and delete intent/unlink/directory-sync/completion
boundaries; production ignores them. See `docs/proofs/15-explicit-history-cleanup.md`.

### Sustained measurement boundary (ticket 17)

The storage worker retains a bounded 1024-entry ring of data-bearing durability
completions: sequence, committed raw prefix, worker-relative completion time,
actual transcript sync plus SQLite commit duration, and oldest accepted age.
Sequence gaps reveal overwritten observations; polling only the last completion
cannot establish a full interval distribution. A failed append does not mark the
active file dirty or mint unchanged-prefix completions. The oldest pending
accepted time survives until a successful append/commit. Metadata excludes this
transient ring.

The developer proof uses one paced scripted producer alongside bounded stdin
collection, short stdout locks and explicit input receipts before ACK output.
Ordinary stdin bytes do not flush/acquire stdout; one complete ACK can still wait
behind real output backpressure. Its bounded continuous outer-terminal parser
measures actual injection-to-parsed-visible feedback. The proof records source
and binary boundaries, same-session handoffs, RSS by process, actual queue
saturation, disk growth, complete commit sequences and retained prefixes. Full
storage can freeze native output while dashboard recovery/Stop remain available;
a receipt is distinct from a visible usable native interaction. See
[the final measurements and limits](proofs/17-session-measurements.md), including
the independently declared fault-input supplement and preserved failed runs.

### Integrated evidence boundary (ticket 18)

The [final proof record](proofs/18-milestone-evidence.md) ties native observations
to source, exact CLI versions, isolated PTYs/state, raw cell captures and required
checks. Whole-dashboard160×40 and60×18 captures follow the reference composition,
with unavailable reviews, one active session per worktree and the native CLI pane.
Narrow layouts hide the sidebar; unusable viewports retain the last usable child
geometry. Parsed-cell rasterization does not establish Terminal.app/iTerm
compatibility.

An actual Codex0.167.0 foreground command created a separate native child process
group. The pre-fix Stop confirmed the CLI group but left that command alive, so
owned native subprocess cleanup and ordinary quit required a correction. The
fresh native retests, final source checks and live-tracker resource revalidation
all passed. This concrete default-native behavior is distinct from
arbitrary processes deliberately escaping ownership. The record preserves both
the original failure and the final fix/retest boundary when available.

### Final review interaction and lifecycle boundaries

`events` defines the canonical dashboard effects, session requests, observations,
launch results and lifecycle vocabulary. `App` accepts usable dimensions from
`Ui`, remembers successful resize dispatch for the exact control target, managed
session UUID, execution readiness and dimensions, and retries a saturated bounded
queue on the next frame. A previous owner cannot satisfy a new owner's resize;
the adopted parser receives its viewport before launch, and a newly running PTY
receives it again if preparation already accepted that geometry. `main` only
dispatches that effect and returns its result.
The launch-intent helper shares request identity policy across explicit fixture,
Claude, Codex and resume actions. Persistence commands retain their separate
prepare/authorize/save meanings while carrying one named metadata snapshot.

New observations and durable JSON carry a typed task/session/request identity.
Session UUIDs key capture, history sealing and cleanup; the existing wire display
label remains derived for compatibility. Old durable JSON is converted explicitly
at the persistence read boundary with bounded task/request/label lengths and UUID
validation. Opaque task and request strings remain valid, including request spaces;
malformed legacy identities fail visibly without deleting records or history.

A native leader's original exit code/signal is retained before owned-descendant
cleanup. Confirmed cleanup produces `Exited` for success or `Failed` for nonzero
exit/signal; explicit Stop/quit of a running process remains `Stopped`. Cleanup
failure remains `StopFailed` and retryable without discarding an already observed
native outcome. Semantic turn status remains separate. Failed final executions
follow the same bounded drain, final flush, seal and explicit-next-launch barriers
as successful exits. The original reason is persisted independently of transient
control/storage errors and returned with readable history; reload never turns a
fully cleaned failed session into uncertain execution or restarts coding.


### Milestone 2 durable-operation attribution (ticket 01)

Final flush and history sealing use the same attributed completion boundary.
Shutdown waits for every pending operation, and final flush must settle before
finished capture is replaced. Capture obligations remain distinct from the wire's
metadata-pending flag. Sealing becomes
confirmed only after its matching commit, rather than when its command is queued.
A failed launch-result write remains provisional through unrelated metadata saves.
Explicit storage retry saves the current stopped/running facts under a new result
operation, without authorizing a new native launch. A provisional result retains
its request slot, so another launch/query cannot replace it before completion.
A successful metadata retry clears only its own diagnostic, preserving newer
execution/input errors.

This change keeps the existing public Request/Observation representation and
SQLite schema. Operation UUIDs are private in-process writer identities, not a new
wire protocol or a durable replay log. The sole-owner lock is acquired before
opening/migrating storage; an existing live owner is never migrated underneath or
replaced. Existing same-request launch outcomes and conservative restored process
evidence remain the restart boundary. Therefore this ticket introduces no
incompatible message to send to a live Milestone 1 owner. Later changes that add
requests or migrate records must detect incompatible live owners before using
those capabilities; unchanged wire compatibility does not establish that future
handshake. See [the ticket evidence](proofs/m2-01-durable-session-operations.md).

### Prompt-led native launch and local workspace labels (Milestone 2 ticket 07)

The explicit optional-first-prompt action sends `LaunchNative` through the same
prepare/authorize/exec-receipt/result boundary as existing native launches. Its
prompt survives only the live pending/creating request, becomes one positional
native argv argument after `--`, and is never a restoration/reconciliation replay.
Resume does not supply a prompt. Workspace labels are local metadata: schema 5
records generated fallback, first-prompt, manual and retained provenance so later
conversations cannot overwrite a manual or earlier meaningful name. Preparation
commits a prompt-derived default atomically; explicit rename uses the sole storage
worker and announces only its committed registry snapshot. Names do not change
folder, branch or workspace/native-session identity. No native title is guessed
from screen text and no extra naming turn/provider is used. See the
[versioned first-prompt/naming proof](proofs/m2-07-prompt-naming.md) for actual
Claude 2.1.319 / Codex 0.167.0 results and remaining qualification boundaries.

### Working inspection (Milestone 2 ticket 13)

Working inspection is a workspace-owned request, independent of the selected
coding runtime. The supervisor resolves its stable workspace ID through the
retained registry and admits at most one cancellable comparison job. That worker
rechecks canonical checkout identity, reads NUL-delimited status and generates
separate HEAD-to-index and index-to-working-file patches; new files are read
without staging. It shares the existing bounded Git subprocess runner (two seconds
and 256 KiB per command, owned process-group cleanup), disables optional index
locks, fsmonitor, external diff and textconv, and never takes mutation authority.
Stop/Shutdown cancel it without waiting in the control loop; shutdown retains the
worker’s cleanup obligation. Whole workspace and revision-pinned Review remain
separate comparisons.

Snapshots retain workspace/request IDs, canonical checkout identity, capture time,
raw file paths, staged/unstaged/new/conflict state and selected content. One
snapshot presents at most 1,024 files and 64 KiB per patch/new file, with explicit
omission/oversize labels; the Git output cap produces an explicit unavailable
comparison when status itself exceeds the bound. Submodule nested content is
outside this comparison and is labelled. Safe printable text escapes terminal and
bidi controls. The dashboard discards stale workspace/request responses, preserves
file/context across refresh when possible and labels snapshots as not live.
Status changes during capture get an additional warning; matching status does not
claim atomic file contents. Each new inspection starts Working. See the
[ticket evidence](proofs/m2-13-working-diff.md).


### Independent workspace creation (Milestone 2 ticket 09)

`creation` owns the concrete creation coordinator and one bounded Git worker at a
time. The supervisor owns this coordinator, and a separate `creation_protocol`
greeting prevents an older live owner from receiving unsupported requests. The
dashboard chooses project/base/branch/folder/agent and an optional first prompt;
Enter means Create without starting, Ctrl-S means Create and start. Generated
folders use stable project/request UUIDs beneath execution-side product storage,
outside the original checkout. Unix path bytes survive suggested-path display.

The sole writer's schema 6 creation receipts bind immutable requested intent to
its operation UUID, canonical common directory, original base ref/commit,
creation phase and optional resulting checkout. Request identity is also the
workspace's retained task identity and the native-launch request identity. The
writer atomically registers the completed checkout and prompt-derived name with
its receipt, and seeds an independent selected comparison base without replacing
a later user selection. Original creation facts remain on the workspace.

An intent commits before fetch; the exact fetched or explicitly chosen local
commit commits before branch/worktree creation. Git atomically creates the new
branch together with `refs/rat/creation/<operation UUID>` at that commit. The
private claim lets a retry finish an operation-owned branch after partial Git
success, while a branch without that claim remains an external collision. The
claim retains the original base independently of mutable visible branch names.
The bounded Git runner accepts at most 512 bytes of transaction input through a
new owned stdin pipe; its existing cancellation, output and child-reaping bounds
still apply. Fetch and creation hold the
per-common-directory Git exclusion, use bounded output and owned process groups,
and have a 30-second child deadline with cancellation. Read-only Git identity
checks retain their two-second bound. Git refuses existing branches and folders;
the worker rechecks identity and destination before mutation. The original index
and working files are never used to construct the new checkout.

A failed fetch commits LocalChoice without creating anything. Its explicit local
continuation uses the already displayed commit, even if a tracking ref has moved.
Cancellation disarms launch and retains partial Git results. Its request freezes
the creation UUID and available original native session UUID. After the checkout
handoff it also sends Stop only to the owner whose workspace task and launch
request still match that creation, so an old receipt cannot stop a later
conversation. Recovery uses the
same operation and exact target; it recognizes an existing checkout only when
common directory, operation claim, branch and pinned HEAD match. Ambiguous partial branch/folder
state is retained and refused rather than overwritten. Store/transport failure
never grants the next phase; matching attributed acknowledgements do. Saved
receipts alone never run Git or native code on restore. Explicit reconciliation
disarms launch; only a fresh Create and start or a clearly labelled local-code
continuation can arm it. The existing native owner retains durable same-request
launch outcomes, independently of the completed checkout receipt.

Completed workspaces pass through `LaunchHandoff { record, authorized_start }`.
`record.intent` remains the original immutable durable request; later setup code
may persist that record unchanged. `authorized_start` is transient launch intent
and must never be written back into `record.intent.start`. Completed workspaces
enter the existing session collection without selecting that owner. A requested launch freezes that owner's target/session identity, so
late completion cannot steal navigation or launch in the currently selected
workspace. Receipts project the originating owner's pending or durable native
result. Receipt projection is keyed by the creation's original native request,
not the owner's current conversation. A later conversation cannot replace its
outcome, original session UUID or final process presentation. Startup collects
bounded typed historical receipt metadata from the existing SQLite scan before
older stopped rows are omitted from runtime ownership; this read-only projection
adds no AgentSession, execution reservation or selection change. Failed native
start keeps the registered workspace, branch, folder and history; retry remains
an explicit Start. Configured setup now owns the intermediate
execution/continuation boundary described below. See the
[scoped evidence and native limits](proofs/m2-09-workspace-creation.md).
### Whole workspace inspection (Milestone 2 ticket 14)

Working remains the default for each new inspection. Within that view, an explicit
switch requests Whole workspace through the same workspace-owned bounded worker.
The supervisor resolves the retained checkout and selected local base; an adopted
workspace without a base requires explicit selection of a full local branch or
remote-tracking ref. Inspection never fetches. Multiple merge bases, unrelated
histories and unavailable refs produce explanations rather than guessed bases.

The selected base's raw ref bytes and selection-time commit live in the additive
`workspace_comparison_bases` table, separately from current branch and immutable
workspace-creation facts. Explicit selection resolves the ref in the Git worker,
then the sole persistence worker commits the choice under an attributed operation.
Only that completion publishes the successful snapshot. Failed saving preserves
the previous durable choice and returns an attributed inspection error. Creation
may seed the comparison choice from its original base with INSERT OR IGNORE;
subsequent comparison choices never overwrite original creation facts.

Each Whole snapshot names the captured branch, HEAD, actual current local base tip
and unique common ancestor. Base movement is labelled against the selected commit.
The file list combines committed changes since that ancestor with current status,
including untracked files. Selected content compares the ancestor with current
working files and retains separate staged/unstaged sections where present, so
staged content remains inspectable even if later edits undo it. Conflicts remain
explicitly unresolved; submodule nested files are outside the comparison. Raw
Unix paths, cancellation, Git/process/output caps and external-program exclusions
reuse Working's boundaries. Ref/branch/status changes observed during capture add
a warning; snapshots do not claim filesystem atomicity.

Comparison protocol 2 is required before sending Whole fields: protocol 1 would
ignore them and incorrectly return Working. An incompatible live owner remains
untouched and requires its compatible dashboard for stop/upgrade. These are local
coding comparisons. They neither pin future Review revisions nor substitute an
archive recovery ref for a review base. External-editor handoff is ticket 15.


### External editor inspection and live source (Milestone 2 ticket 15)

`InspectWorking.editor` requests a fresh full Working/Whole workspace artifact
from the existing cancellable comparison worker. `editor_protocol=2` advertises
that capability separately from comparison protocol 2; a dashboard connected to
an older owner refuses editor inspection rather than treating a selected-file
snapshot as a full export. Content and identity travel in `Snapshot.editor_content`,
not an execution-side temporary filename. The export includes the file inventory
and per-file state/reasons, with 64 KiB per file, 1 MiB total, 1024 inventory entries
and a five-second budget checked between files. Existing bounded Git process
commands and cancellation remain in force; one command/file can finish after
that between-file budget. Long display path labels are abbreviated at 256 bytes;
raw path identity remains in the snapshot. The worker records the selected raw
path's one-based artifact heading in `Snapshot.editor_line`, including omitted
sections; editor preparation validates that bound rather than matching display
labels. `Snapshot.source_start` records the zero-based start of plain current-file
text for new/conflicted files, including Whole workspace headers. Patch source
positions continue to follow new-side hunks. These optional scalar fields preserve
bounded payloads; editor actions refuse older capability versions lacking this
position contract. A local per-file presentation helper is shared by Working,
Whole and editor export; their comparison headers and export caps stay separate.
Matching status is not atomicity.

`EditorSource` freezes request/workspace/path/line and runs retained-checkout and
existing regular-source validation in an execution-owned cancellable worker.
Responses are accepted only for the matching request/workspace. The current
local supervisor returns an explicit host-local source path; this does not grant
future remote owners a dashboard-local file editing capability. A file removed
externally after validation may still race the editor's own open; the application
never deliberately invents a missing source path.

The dashboard's preparation worker materializes inspection text in a unique
0700 temporary directory outside the checkout, marks the artifact read-only and
keeps it alive through editor exit. It does no checkout Git or source reads.
Cleanup runs off the UI loop. Configuration is executable plus literal argv in
`RAT_EDITOR_JSON`; `Command` executes it directly. TerminalGuard releases raw,
alternate-screen and paste ownership. A dedicated dashboard editor worker owns
spawn/wait and assigns the editor its own foreground process group; main suppresses
input/render while polling completion. This keeps native Ctrl-C within the editor.
TerminalGuard reclaims the dashboard foreground group and raw/alternate/paste modes
on success/nonzero/spawn failure. Fullscreen resize resets rendering buffers at the
actual geometry without a cursor-position query.
The App remains intact through handoff, retaining project/workspace/session,
focus and diff/history context. The independent supervisor continues capturing
agents. Inspection is labelled snapshot; source/workspace editing is labelled
LIVE and shared with the agent. GUI editors require their configured wait option.


### Configured project setup (Milestone 2 ticket 10)

Schema 8 follows schema 7 and adds optional project setup configuration, frozen
creation setup policies and independent setup attempt receipts. Only the existing
persistence worker writes them. A new creation intent atomically freezes the
optional project shell command with its original creation facts. Missing legacy
policies are not retroactively guessed. If creation completed before its first
setup receipt committed, restore saves a known-not-started failure receipt and
requires explicit Retry or Continue. It does not run code. Existing setup receipts
retain their original attempt UUID, command, creation/task/canonical checkout,
boot/start identity, phases, bounded tail and cleanup facts. Native conversation
membership and original native launch outcomes remain independent.

`setup::Coordinator` commits Prepared before worker spawn and exact attributed
Authorized before releasing the authenticated local gate. The worker installs the
std child immediately, sets up nonblocking pipes, creates a fresh session/process
group and live descendant Scope, then verifies original gated identity before
release. Gate EOF is the exec receipt; a fast command may already have exited, so
post-EOF leader survival is not required. Running and terminal facts pass through
the same sole writer. Success may grant native launch only after its matching
terminal commit, confirmed execution cleanup and still-live transient launch
permission. The original `LaunchHandoff.record.intent` is never masked or mutated
to encode that permission. Failure blocks bare Launch/Resume until explicit retry
or skip. Retry creates a fresh attempt, retaining all prior outcomes and the
frozen command; a changed project setting cannot rewrite it.

Setup has one concrete execution worker, not an AgentSession or generic job
engine. It runs unrestricted local coding-workspace setup through `/bin/sh -c`;
this grants no future restricted reviewer capability. One setup is admitted at a
time. New configured setup also counts against the 32 unsettled execution/capture
admission limit. Canonical common/git directory reservations include pending
writer acknowledgements, cleanup/output tails, known failures awaiting choice
and uncertain restoration, with only the latest attempt for each creation
participating. Path aliases cannot bypass native admission. Shared-workspace
mutations must cancel pending launch permission and account for these setup
receipts as well as native owners; an empty/stopped native owner proves no setup
cleanup. No archive/removal action is introduced here.

Targeted Stop cancels the matching setup plus its pending launch permission even
when its coding owner is still empty. A frozen empty-owner Stop may follow only
that same creation's original native handoff if a native UUID appeared before
command delivery; later independent conversations remain protected. Global Stop
and quit cancel owned setup and wait for durable result/capture/cleanup. Detach or
controller loss preserves supervisor-owned setup. No restarted supervisor signals
restored PIDs. Released execution without confirmed cleanup uses the existing
conservative read-only reconciliation; absent leader is insufficient to exclude
escaped descendants on the same boot. Unresolved survivors prevent quit's stopped
acknowledgement. A durable terminal `cleanup_confirmed` fact separately permits an
explicit informed retry/skip when only command outcome is uncertain. Reconciliation
never clears historical `Execution.released` facts or restores launch permission.

The command limit is 4096 bytes; there are at most 32 retained attempt receipts,
each at most 64 KiB serialized. Two nonblocking reads of at most 4096 bytes per
pipe per poll bound capture work. A coalesced 8192-byte combined raw tail plus
explicit discarded count bounds progress without producer backpressure or fake
conversation history; terminal controls are inert. Live tail changes are transient
until a phase receipt commits, independently of native history flush guarantees.
Gate connection/exec receipts
have five-second deadlines. Arbitrarily slow setup has no inferred completion or
short total timeout; cancellation is polled at 10 ms intervals. Existing lifecycle
Scope closure/TERM/KILL verification runs away from routing and retains child
ownership on unconfirmed cleanup. Final pipe drain has a 250 ms bound; incomplete
capture yields uncertainty rather than success. Storage failure retains the exact
phase for retry without replaying external effects. A lost worker becomes a
conservative uncertain receipt. Full process-tree sampling/reboot/native limits
remain those of the existing ownership foundation; see the
[scoped setup evidence](proofs/m2-10-project-setup.md).


## Stop, save and archive (Milestone 2 ticket 16)

`archive::Coordinator` owns one current explicit request and attributed durable
phase receipts. `archive_git` runs bounded cancellable Git subprocesses in the
execution worker and acquires `RepositoryMutation` only for snapshot/Git work.
Waiting for Stop, setup cancellation or SQLite receipts holds no repository
mutation lease. This introduces no generic job framework.

Admission freezes on canonical common-dir/git-dir, independently of selection
or display labels. Every relevant session owner receives Stop, including retained
workspace task aliases. Archive readiness requires no execution/launch/uncertainty
reservation, no capture tail, no durable-write obligations or metadata failure,
and no outstanding launch result. Setup's checkout reservation also must settle;
a stopped label or global pending count cannot authorize Git. Pending creation
and setup handoffs are disarmed. Selected native resume re-enters the same
admission gate after its asynchronous descriptor lookup; Stop also durably
cancels that lookup request. Archived visibility continues to block Start.
Admission distinguishes recovery still loading, a pending mutation receipt,
a settled archive requiring Return/unarchive, and a completed folder removal
requiring recovery into an available checkout before coding. Completed receipts
never imply that native resume or a new session can run in a missing folder;
readable history stays available. Diagnostics use retained owner facts without
filesystem, Git or storage work in the UI loop.

Schema 10 stores `workspace_archives(request,workspace,record)`. Waiting intent
is durable before snapshot preparation; Prepared binds checkout, symbolic branch,
pre-HEAD, combined working tree, original staged tree and real-index byte hash
before commit. Request reuse cannot change those prepared facts. Saved Git and
recovery protection precede the single SQLite transaction connecting the receipt
and archived visibility. Only its matching completion permits success. Restart
loads incomplete receipts without executing Git or coding; explicit retry uses
the same prepared request. Commit recognition checks parent, tree and random
operation-bound message together, never message text alone.

The worker owns `index.lock`, keeps immutable original/combined alternate indexes,
and runs ordinary `git commit` with the user's identity/hooks/signing settings.
A workspace-specific ref pins the original staged tree (`-index`) and combined
prepared tree (`-prepared`); a commit ref pins saved HEAD. These retain staged-only
blobs and code through GC independently of the visible branch or transcript.
After parent/tree/operation identity, canonical checkout/branch/HEAD, working
snapshot and original real-index hash rechecks, the worker atomically replaces
its index lock with an index matching saved HEAD. Error paths do not restore or
overwrite the real index. A partial commit/ref remains local and explicitly
unarchived when observed concurrent/hook edits invalidate a complete save.

Archive Git commands request `core.fsync=committed,reference`, `core.fsyncMethod=fsync`.
Index artifacts and containing directories are explicitly synced; SQLite retains
FULL synchronous/fullfsync settings. This is ordered protection across two stores,
not a cross-store atomic transaction or a tested physical power-loss guarantee.
A crash after Git but before metadata replays the same durable Prepared receipt;
uncertain surviving Git locks are never deleted automatically. Per-command Git
output stays capped at 256 KiB with a 30-second mutation deadline; receipts are
bounded to 64 durable operations and 32 KiB per record, refusing excess without
claiming archive success. Refs are not automatically expired.

Immutable original creation branch/base ownership remains in Workspace creation
facts; prepared branch and recovery reference remain in the archive receipt.
Ticket 17 adds branch cleanup/restoration below; ticket 18 owns separately
confirmed folder removal against exact current HEAD.
See [ticket 16 evidence](proofs/m2-16-archive.md).

## Archive branch cleanup and return (Milestone 2 ticket 17)

Schema 10's bounded JSON receipt gains backward-defaulted immutable deletion
selection, original owned branch and selected base plus a separate `BranchPhase`
and message. No schema/table or generic scheduler is introduced. Product-created
branches default to deletion; adopted branches require the exact previewed target.
Prepared snapshot and policy cannot change on request reuse. Saved OID becomes
immutable once recorded. Protocol 2 binds Return to workspace **and archive
request**, rejecting stale revisions and treating an already-returned receipt as
a no-op, including when the active checkout has since become dirty.

A durable Archived/CleanupPending receipt precedes the cleanup worker. This
worker alone acquires the common-dir mutation lease; no lease spans SQLite or
Stop. It revalidates canonical identity, saved recovery ref, direct local branch,
immutable creation provenance, expected saved HEAD/ref and clean files/index.
Main/master/trunk, selected local/remote base name and remote-default branch names
are protected. Ordinary `switch --detach --no-overwrite-ignore
--no-recurse-submodules` preserves checkout semantics and hooks. Protected/default,
direct-ref, hooks, tip, recovery and checkout-use facts are checked again after
detach. Failure retains the branch, folder, ignored files and confirmed archive;
detach success is not silently rolled back.

Deletion uses **ordinary `git branch -D`**, preserving its branch-in-use refusal.
A command-local `reference-transaction` prepared guard checks the direct ref's
actual OID under Git's lock, rather than trusting the zero old OID of an
unconditional transaction. It checks saved recovery, current default names and
worktree occupancy too. No low-level deletion fallback exists. The qualified
subset is Git 2.50/files refs with no custom hooksPath or executable transaction
hook; other policies/backends/versions fail closed. Hook policy is checked before
and after detach and in prepared. Other ordinary checkout hooks remain effective.
The guard argument and environment preserve raw Unix paths. App mutation leases
exclude cooperating app workers only; worktree-use checks remain immediate
snapshots. Ordinary external `worktree add --no-checkout` can race Git's own check;
this is not a universal interprocess checkout lock or branch-generation identity.
Direct Git-file edits and unobservable same-tip delete/recreate are outside that
bounded guarantee. Observable branch replacements/advances are retained.

Return durably records RestorePending before bounded Git work. Recovery and clean
checkout identity must still match. Absent original names can be recreated;
occupied names receive a fresh operation-derived alternative. A create-only
`update-ref --stdin` transaction creates the branch and its operation marker at
`refs/rat/restore/<workspace>/<archive request>/<branch>` together. Marker/ref
verification reconciles a crash after create or switch without overwriting reused
refs. Ordinary safe switch preserves user hooks. Only a matching durable
Restored receipt sets archive visibility to false in the same SQLite transaction.
Missing folders, changed files/HEAD/recovery, hook failure or branch-in-use refusal
remain archived with actionable partial outcomes. No native start is dispatched.
Workspace, native conversation and future PR/review identities do not derive
from restored branch-name equality.

Cancellation remains disarmed through late durable intent acknowledgement.
A cancelled running Return retains any Git result and archived visibility.
Once an actual completed branch result is already queued for durable storage,
cancellation reports that it is too late rather than promising a rollback.
Restart loads pending receipts without automatically mutating Git; explicit retry
reconciles absence as uncertain, preserving the archived saved point. Cleanup and
restore have distinct storage fault boundaries. See [ticket 17 evidence](proofs/m2-17-branches.md).


## Explicit workspace folder removal (Milestone 2 ticket 18)

`removal::Coordinator` is a separate, bounded concrete operation owner. A read-only
Git worker returns an immutable preview for a workspace/request UUID. Schema 11
stores `workspace_removals(request,workspace,record)` through the existing sole
writer, retaining at most 64 receipts of at most 32 KiB each. Receipt storage does
not alter archive visibility, transcript tombstones, native IDs, creation facts,
branches or future PR/review identity. The independent removal protocol capability
(version 1) rejects incompatible live supervisors before dashboard dispatch.

Explicit confirmation names the stored HEAD and recovery ref, and acknowledges
local-only code when applicable. The supervisor rechecks every relevant canonical
session owner and workspace task alias: execution/launch lookup uncertainty,
capture tails, durable metadata/outcome obligations and setup/creation handoffs
all block confirmation. Incomplete archive/branch mutations also block it.
Confirmed removal reserves the canonical common-dir/git-dir during writer waits
and Git work; aliases and asynchronous native-resume lookup completions re-enter
the same admission gate. Archive/Return and setup retries cannot bypass this gate.
Removal does not implicitly Stop a session or setup; independent Stop remains
available to settle those blockers. Quit cancels pending mutation intent.

After the frozen Preview receipt commits, explicit confirmation commits Intent.
Only its exact completion starts `removal_git::protect`: verify the linked-checkout
identity and eligibility, then create-only or verify the exact current HEAD ref at
`refs/rat/removal/<workspace>/<request>`. Reference writes request committed-object
and reference fsync. The complete snapshot/ref/HEAD and Removing intent then commit
through SQLite before a separate Git worker may invoke ordinary `git worktree
remove -- <canonical root>`. These two workers acquire the common-directory
mutation lease only for bounded Git work, releasing it before SQLite waits.
No force, branch deletion or recursive recorded-path deletion is available.

Workers preserve raw Unix path bytes and anchor root, Git/common directories,
`.git` link and worktree registration with device/inode evidence. Main, missing,
locked/prunable, replaced or unsupported identities are refused. Tracked/untracked
status and an independent ignored-file inventory are checked at preview,
protection and immediately before removal: ordinary Git removal itself can delete
ignored files. Git operation/lock state, sparse/special index entries, submodules,
shallow/partial clones and external object dependencies conservatively block
unsupported recovery coverage. HEAD/branch, recovery ref, canonical links and
registration repeat after remote/ref work and immediately before deletion.

Push evidence uses bounded live `ls-remote` checks of configured push destinations,
with prompts disabled, at most eight destinations and a 12-second inspection
budget (each subprocess retains its two-second deadline and 256 KiB pipe bound).
Missing/inaccessible configured upstream/destinations are uncertainty and block;
known absence of remotes establishes local-only saving with an explicit
acknowledgement. Without an upstream, live destination evidence must independently
cover HEAD to report remote backup; otherwise acknowledgement remains required. A local tracking branch is not remote backup. Preview
records a fingerprint of configuration/advertised refs; changed evidence requires
a new preview. No fetch, push, credential copy or remote ref mutation occurs.

Success is visible only after the matching durable Removed receipt. Lost storage
acknowledgements, cancellation or uncertain Git outcomes retain the recovery ref
and show uncertainty; no next deletion step is armed. Restart only loads receipts.
Explicit `g` launches read-only reconciliation: absent root plus absent registration
and the exact protected commit confirms removed; matching anchored retained state
confirms retained and permits a fresh request. Replacement/inaccessible identities
remain uncertain. It never replays removal or unlinks a replacement path. Saved
commits and previous archive refs survive GC; history/search remains independent
of checkout availability and in-place native resume is explained as unavailable.

The process-local lease excludes cooperating application workers, not arbitrary
external editors/Git. Final checks and ordinary Git refusal bound observed safety;
there is no universal external-write atomicity or physical power-loss guarantee.
Git filters/LFS retain ordinary Git semantics for external resources. See
[ticket 18 evidence](proofs/m2-18-removal.md). Declared M2 coding-only native/workload,
sleep/wake and physical reboot qualification is recorded in ticket 19
and the [actual reboot evidence](proofs/m2-19-reboot.md).


## Integrated coding qualification corrections (Milestone 2 ticket 19)

The capture writer takes a bounded turn for each stream before the existing
512-byte derivative-index step: at most 64 blocks/64 KiB, ending after a
completed write reaches 2 ms. A write can itself block beyond that budget. This
avoids paying an index checkpoint for every small native PTY append while
retaining rotation, queue bounds, control priority, committed-prefix durability,
index transaction atomicity and FULL/fullfsync. The source-pinned qualification
records fair raw capture under six streams; derivative indexing remains partial
under that workload, so durable raw history is not an immediate full-search
completion guarantee.

An explicit launch/resume action on a terminal historical segment gets a fresh
request even when its old launch outcome is absent. Pending/uncertain execution
and retrying an unobserved new intent retain their existing request identity.
Historical native lookup verifies selected membership and identity, and reports
a known missing current workspace before native-descriptor rejection; unknown
legacy locations still allow identity-only lookup. Neither path changes the
saved native conversation UUID.

The [integrated qualification](proofs/m2-19-qualification.md) preserves the failed
first aggregate, regression evidence and clean corrected aggregate, separate
fixture/native boundaries, real archive/removal/local recovery, and exact
version limits. Native crash restoration can retain a conservative same-boot
Interrupted reservation after its processes are gone; the proof preserves it
rather than clearing ownership from saved PIDs. Final review/fixes and the frozen
all-target integration passed. Actual user-performed sleep/wake and reboot then
verified identity/code/history preservation and zero automatic coding. Restored
reservations required existing explicit reconciliation before original-ID resumes;
no saved PID was signalled. Both marker recalls, Stop/durability and ordinary
authenticated Shutdown passed. The recorded M2 C6 gate is complete; later release
gates and version/workload limitations remain separate.
