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

**Git/worktrees (`git`).** A dedicated worker runs `git -C <path> worktree list
--porcelain -z`, parses NUL-delimited fields, and sends a completion event.
Arguments are passed directly to Git, without shell interpolation. Discovery is
read-only, handles linked/detached/bare/locked/prunable worktrees, and preserves
non-UTF-8 Unix paths internally. The bounded request queue and one in-flight
refresh prevent repeated keystrokes spawning processes. Errors remain visible
and retryable. Future mutating operations need explicit commands, dirty/unpushed
checks, bounded execution/cancellation, and serialized operations per repository.
The current discovery worker can wait on a stuck Git process, but does not block
the UI or dashboard exit; process deadlines belong in the next worker hardening.

**Agent sessions (target; scripted prototype below).** A local supervisor process, implemented as another
mode of the same executable, must own child processes, PTYs, and history. The
dashboard is a reconnectable client over a user-private local socket. There is
one supervisor per macOS user, shared across projects, and at most one connected
dashboard controls it at a time; see
[the ownership decision](adr/0001-user-wide-supervisor.md). This separation is
necessary because the product requires agents to survive terminal
or dashboard loss. The scripted prototype below implements supervisor/session
ownership; durable persistence and crash reconciliation remain planned.
The supervisor owns one active session per worktree, stable session IDs, launch
deduplication, lifecycle transitions, and explicit stop versus detach. Ordinary
quit requests supervisor shutdown; an unexpectedly lost client does not. On
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
compatibility still requires recorded macOS evidence. This gives locality to
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
`StopFailed`) and separate from semantic agent status, which remains unknown.
The supervisor polls child exit even without a controller; native exit retains
in-memory attribution and the bounded screen. Stop keeps the dashboard open;
quit enters App pending shutdown and exits only after a confirmed observation.
A stop failure preserves the child reservation, shows an actionable error and
allows explicit retry. Final shutdown observations use reliable event delivery
rather than the coalescible screen-only path. Detach exits through the normal
terminal guard without sending Shutdown. Client EOF merely releases control.

A termination worker owns the child while stopping, sends process-group TERM,
escalates after 500 ms and polls leader exit plus `ps` group execution evidence.
The deadline is two seconds with a 500 ms timeout on each verification subprocess.
Inspection failures never count as confirmed absence. Zombies cannot execute;
processes that deliberately escape the managed group are outside this proof.
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
for versioned results and unresolved compatibility gates.


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
