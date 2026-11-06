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

**Agent sessions (next).** A local supervisor process, implemented as another
mode of the same executable, must own child processes, PTYs, and history. The
dashboard is a reconnectable client over a user-private local socket. There is
one supervisor per macOS user, shared across projects, and at most one connected
dashboard controls it at a time; see
[the ownership decision](adr/0001-user-wide-supervisor.md). This separation is
necessary because the product requires agents to survive terminal
or dashboard loss. No supervisor/session code or persistence is present yet.
The prototype must validate this choice before other features rely on it.
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
