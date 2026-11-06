# Milestone 1 — native-session proof

Status: ready-for-agent

## Problem Statement

A developer can currently discover and navigate a repository's worktrees, but
cannot use the dashboard to run a coding agent. Before depending on session
coordination, they need proof that the dashboard preserves the agent's native
interaction, identifies work needing attention honestly, survives dashboard
loss, and retains readable session history without becoming slow or consuming
unbounded resources.

The highest-risk boundary is a real interactive CLI on macOS. Simulated output,
a process that merely launches, or a simplified chat interface cannot establish
that Claude Code and Codex CLI are usable through this product.

## Solution

Deliver one usable managed coding session in one existing Git repository.
Discover its worktrees without mutation, explicitly adopt a checkout, select an
available native agent, launch it, and attach to its native terminal. Let the
developer return to dashboard navigation while work continues and reconnect to
the same session after losing the dashboard.

Begin with a PTY/supervisor prototype and carry the demonstrated behavior into
the usable dashboard slice. Use the reference image's reviews-above-project-tree
sidebar, prominent right-hand session area, dark bordered presentation, clear
status labels, and visible keyboard actions from the first usable slice. Show
reviews as unavailable at this milestone. Preserve native agent input and screen
behavior inside the session area rather than implementing the image's example
message composer.

Completion requires recorded native macOS evidence for each claimed supported
CLI version, repeatable failure/recovery scenarios, and measured responsiveness,
history durability, memory, queues, and storage behavior. An unresolved native
compatibility problem remains a reported proof failure requiring a decision;
fixtures cannot waive it.

## User Stories

1. As a developer, I want to open the dashboard in an existing repository or pass its path, so that I can work in a checkout I already use.
2. As a developer, I want discovery and refresh to remain read-only, so that browsing never starts coding or changes my repository.
3. As a developer, I want to explicitly adopt a worktree and see its branch and path, so that I know where the agent will work.
4. As a developer, I want to select an available Claude Code or Codex CLI configuration using my existing authentication, so that I can use my own tools and account.
5. As a developer, I want missing executables and startup failures explained with recovery guidance, so that I can correct setup without losing context.
6. As a developer, I want a stable task and session identity associated with the adopted worktree, so that output and history remain attributable after restart.
7. As a developer, I want a second active session in the same worktree refused, so that managed agents cannot concurrently edit that checkout.
8. As a developer, I want attach to enter the running session rather than launch another process, so that I continue the same work.
9. As a developer, I want to answer real native prompts and approvals, so that the agent remains usable without a replacement conversation interface.
10. As a developer, I want native shortcuts, Unicode input, and paste to reach the agent correctly, so that terminal interaction remains familiar.
11. As a developer, I want Ctrl-C in attached focus to retain native interrupt behavior, so that I can interrupt the agent without quitting the dashboard.
12. As a developer, I want a documented escape chord to return to dashboard focus, so that I can navigate without stopping the agent.
13. As a developer, I want navigation and refresh to stay responsive during sustained output, so that busy agents do not monopolize the dashboard.
14. As a developer, I want resizing and reattaching to show a coherent native screen, so that I can trust what I see and type.
15. As a developer, I want explicit focus and visible available actions, so that I know whether keystrokes control navigation or the agent.
16. As a developer, I want the layout to follow the reference image at usable sizes and adapt at smaller sizes, so that the dashboard remains recognizable and operable.
17. As a developer, I want unavailable review features to be shown honestly, so that the layout does not imply implemented GitHub behavior.
18. As a developer, I want process liveness distinguished from agent activity, so that a live but idle process is not reported as working.
19. As a developer, I want input requests and approval requests identified using native-tool evidence, so that I know when the agent needs me.
20. As a developer, I want completed turns distinguished from stopped sessions and archived tasks, so that completion does not hide ongoing work.
21. As a developer, I want interruptions and failures distinguished from intermediate command failures, so that a failing test does not falsely mark the session failed.
22. As a developer, I want unknown status when evidence is missing, so that silence or broken signal capture does not produce a misleading outcome.
23. As a developer, I want unexpected dashboard or terminal closure to leave managed work running, so that a lost client does not end my task.
24. As a developer, I want reopening to recover the same session and screen, so that reconnect never starts duplicate coding activity.
25. As a developer, I want a second dashboard to explain the existing controlling connection, so that two clients cannot compete for input or shutdown.
26. As a developer, I want explicit dashboard detach to preserve activity, so that I can intentionally leave work running.
27. As a developer, I want ordinary quit to stop managed activity and flush history, so that leaving the local app has a clear, recoverable result.
28. As a developer, I want an explicit stop action and native agent exit to preserve my task, history, and working files, so that stopping execution is separate from deleting work.
29. As a developer, I want a lost launch acknowledgement recovered by request identity, so that reconnect does not accidentally launch another agent.
30. As a developer, I want uncertain launches reconciled after supervisor failure, so that a surviving child cannot compete with a replacement.
31. As a developer, I want supervisor failure or reboot to restore interrupted records without restarting coding, so that recovery remains my choice.
32. As a developer, I want readable session history after native exit or abrupt failure, so that retained context does not depend on native resume.
33. As a developer, I want native resume availability explained accurately, so that starting a new conversation is never presented as continuing the old one.
34. As a developer, I want history stored independently of the checkout, so that removing or losing the checkout does not erase saved context.
35. As a developer, I want visible storage errors and bounded buffering without silent output loss, so that I can intervene before history capture fails further.
36. As a developer, I want the dashboard usable for recovery and stop during storage faults, so that backpressure does not trap me in a frozen app.
37. As a developer, I want low-storage cleanup to show session age, task, and size and require explicit deletion, so that valuable history never expires silently.
38. As a developer, I want failed cleanup or storage retries explained, so that I do not mistake an attempted recovery for restored durable capture.
39. As a maintainer, I want reproducible latency and resource measurements with workload and version details, so that the proof supports concrete compatibility and capacity claims.
40. As a maintainer, I want native-tool evidence and fixture evidence reported separately, so that passing CI cannot be mistaken for native macOS support.

## Implementation Decisions

- Keep one Rust package on the stable toolchain. Extend the existing terminal,
  UI, application-state/event, and Git worker boundaries; introduce supervisor,
  session runtime, native adapter, and persistence modules as needed rather than
  new crates or a generic integration framework.
- Use the [interaction baseline](../../docs/architecture.md#native-session-interaction-baseline-planned)
  as the implementation direction, rather than first preserving the foundation
  through a separate refactor. Deepen `App` to own input focus, action decisions,
  pending requests, and observed state through one input/event interface that
  returns explicit effects. Keep the event loop mechanical, blocking execution
  in workers, durable session truth in the supervisor, and viewport geometry in
  `Ui`. Derive visible available actions from the same interaction policy.
  Ordinary quit waits for confirmed stop and history flush; failed or uncertain
  shutdown remains visible and retryable. Bounded effect dispatch reports
  saturation instead of silently dropping input. Keep stop/recovery responsive
  when output or persistence queues are full. Verify these decisions through
  state/rendering checks and the already approved integration seam.
- Prototype the PTY, screen emulation, supervisor ownership, and recovery path
  before building further coordination features. Evaluate a PTY library and a
  terminal parser against real macOS behavior; library selection is an
  implementation outcome, not a compatibility claim.
- Run a user-wide supervisor as another mode of the same executable. It owns
  children, PTYs, stable session identities, launch requests, worktree
  exclusivity, and history through a user-private local socket. Enforce one
  supervisor per macOS user and at most one controlling dashboard. Establish
  election, stale endpoint recovery, permissions, and client-loss detection
  without allowing competing owners.
- Adoption is explicit and records enough project/task/worktree identity for
  this proof. Discovery does not adopt or launch. Resolve checkout aliases to a
  consistent worktree identity before enforcing exclusivity; do not rely on a
  displayed path string or PID alone. Unavailable, bare, or prunable checkouts
  cannot be launched into as though usable; explain the failure.
- Launch uses a stable request identity whose outcome reconnect can query.
  Retrying that request recovers its recorded result. Persist enough intent and
  outcome to identify uncertain launches after failure. Reconcile surviving
  children before clearing exclusivity; unresolved uncertainty blocks replacement
  launch. Do not promise exactly-once process execution or automatically restart
  coding after supervisor loss.
- Launch agents directly in the adopted checkout using existing credentials.
  Avoid shell interpolation of user paths or arguments. Expose only working
  launch choices; missing tools/authentication receive actionable errors. Full
  onboarding and default-setting configuration remain later work.
- Only the dashboard owns the real terminal. Render the emulated child screen
  within Ratatui; never pass raw child escape sequences through to the dashboard
  terminal. PTY spawn, drain, write, resize, reaping, Git operations, and all
  persistence run outside the UI loop. Retain bounded per-frame event work.
- Explicitly model dashboard versus attached focus. Forward native input in
  attached focus, including Ctrl-C, rather than applying today's global quit
  mapping. Choose and document an escape chord during the prototype, verify it
  against both tools, and show it in keyboard help. Include paste, partial UTF-8
  and escape sequences, native prompts, alternate screens, mouse-mode requests,
  and terminal-mode restoration in the compatibility investigation.
- Size the PTY and parser to the actual session viewport, not the entire outer
  terminal. Serialize size changes and synchronize attach through a consistent
  screen snapshot and subsequent output boundary without gaps or duplication.
  Continue draining output without an attached dashboard subject to explicit
  bounded-buffer/storage backpressure.
- Keep transport connection, process lifecycle, semantic agent status, and task
  archival distinct. Demonstrate per-version signals for active work, input,
  approval, turn completion, and interruption in native PTY mode. Surface unknown
  when evidence is absent, invalid, or no longer reliable. Output silence, screen
  text guesses without demonstrated reliability, process liveness, or a failed
  child command alone do not establish semantic outcomes. Signals available only
  in another integration mode do not establish this proof.
- Native exit stops that session; explicit stop terminates managed execution
  with bounded escalation and process-group cleanup. Ordinary dashboard quit
  requests supervisor shutdown and stops managed activity. Explicit dashboard
  detach or unexpected client loss preserves execution. Returning from attached
  focus only changes input ownership. Shutdown failures remain visible rather
  than being reported as confirmed successful stops.
- On supervisor failure or reboot, restore durable records as interrupted or
  uncertain and offer explicit recovery. Preserve working changes. Record native
  conversation identifiers where supported, but retain readable session history
  independently. Exercise supported native recovery in the prototype; a complete
  resume-management workflow follows in Milestone 2. Explain unavailable resume
  and require an explicit choice to start a new conversation.
- Store versioned durable metadata through a single supervisor-side SQLite
  persistence worker in the user's macOS application-support location, outside
  repository files. Use stable identities and bounded append-only transcript
  chunks; retain history references independently of checkout existence. Do not
  copy credentials into product state. Test initial schema migration and crash
  recovery before relying on persistence.
- History includes readable prompts, responses, and captured terminal output;
  establish a replay/readable representation for terminal control sequences,
  redraws, and alternate screens. Reading retained history must not execute
  captured terminal controls or depend on a live child. Show stopped and
  interrupted sessions' retained history without requiring search or archival.
- Target durable streaming flushes at least once per second on healthy storage;
  acknowledge saved metadata only after durable success. Clean shutdown flushes
  pending history and reports failures. After abrupt failure recover the durable
  prefix; explicitly distinguish it from unflushed output that may have been lost.
- Define finite queue, screen, scrollback, chunk, and in-flight persistence
  limits before the sustained-output run. Queue saturation must have an explicit
  policy: preserve transcript data with backpressure, bound slow-subscriber
  delivery and resynchronize screens, and report storage faults. A stopped PTY
  reader does not prove that all agent work or descendants have stopped.
- Low-storage warnings lead to explicit history cleanup showing age, size, and
  task, suggesting oldest stopped sessions first. Never delete based on age,
  archival, or storage pressure alone. Deletion affects selected history, not
  checkout contents. Persistent failure after cleanup remains visible with retry
  and explicit stop available; cleanup is not guaranteed to restore capacity.
- Follow the image's overall composition with one active session per worktree,
  a native screen rather than a product message composer, honest review
  unavailability, and only implemented keyboard actions. Keep selection, agent,
  branch/path, focus, and status readable without color alone. Compare screenshots
  at normal and smaller sizes; document material deviations, consequences, and
  proposed alternatives rather than silently replacing the composition.

## Testing Decisions

- **Confirmed testing seam:** the user approved one primary integration seam:
  drive the real dashboard and supervisor with a scripted native-style PTY child
  in temporary real Git repositories, including crash/reconnect and storage
  faults. Reuse existing state/event and rendering tests for focused behavior,
  then require real Claude Code and Codex CLI evidence on macOS. Prefer this
  external seam over separate mocks for every new module.
- Good tests assert user-visible outcomes: checkout identity, child count and
  cwd, received input/size, rendered focus/status, recovered session identity,
  retained readable history, visible failures, and responsiveness. Avoid tests
  that mirror internal message routing or private data structures.
- Existing prior art is real temporary repository discovery, refresh coalescing
  and selection-preservation state tests, and Ratatui test-backend failure/tiny
  resize tests. Extend these for adoption/exclusivity, focus-sensitive input,
  lifecycle/status events, error preservation, and normal/small layouts. Preserve
  non-UTF-8 path handling and printable display of untrusted metadata.
- The scripted child reports its cwd, identity, received bytes and dimensions,
  emits controllable output and status evidence, and supports controlled exit,
  prompts, alternate-screen transitions, and descendant processes. Split UTF-8
  and escape sequences across writes; exercise Unicode, paste, Ctrl-C, the escape
  chord, resize while attached/detached, and reattach during output. Assert no
  missing or duplicated captured output across snapshot boundaries.
- Use isolated user-state/socket locations so harnesses cannot connect to or
  stop a developer's real supervisor. Exercise simultaneous supervisor startup,
  a second controlling dashboard, stale sockets, dead clients, alias paths, and
  repeated launch/attach actions. Assert at most one managed active child per
  adopted worktree.
- Fault scenarios cover a missing executable, spawn failure, native exit,
  interrupt, explicit stop, descendant cleanup, ordinary quit, explicit detach,
  dashboard kill/reopen, lost launch acknowledgement, and supervisor crashes
  before and after child creation and result persistence. Assert same-request
  outcome recovery and blocked replacement until uncertain survivors are
  reconciled. Restored records must not trigger automatic coding execution.
- Exercise metadata/transcript recovery after clean shutdown and abrupt failure,
  initial schema migration, readable replay, missing checkout, and unavailable
  native resume. Verify durable history independently of the original native
  conversation. Check durable acknowledgements and measure flush intervals with
  a declared durable boundary, not merely successful buffered writes.
- Inject slow writes, write failures, full storage, a slow subscriber, and
  saturated queues at the integration seam. Verify bounded memory, visible
  capture failures, retained accepted transcript data, explicit backpressure,
  responsive recovery/stop, and resumed capture after successful retry. Test
  explicit cleanup and failed cleanup; no automatic history deletion is allowed.
- Status tests distinguish liveness, working, input, approval, completed turn,
  stopped, failed, and interrupted observations. Remove or invalidate evidence
  and verify unknown rather than inferred success, waiting, or completion. A
  fixture validates handling; only real tool evidence validates adapter claims.
- Real macOS checks run separately for recorded Claude Code and Codex CLI
  versions: launch in the adopted checkout, attach, answer native input and
  approval prompts, interrupt with Ctrl-C, complete a turn, escape/reattach,
  repeatedly resize, exit, reconnect after dashboard loss, recover history, and
  demonstrate native resume or its explicit limitation. Record the evidence
  source and limitations for every status signal; fail that tool's support gate
  if dependable required signals cannot be demonstrated in PTY mode.
- Capture terminal screenshots showing the first usable slice, attached focus,
  attention/unknown/error states, and smaller terminal sizes. Compare them with
  the reference image and record differences. Verify terminal restoration after
  normal exit, errors, and Rust panic; Linux checks alone establish no native
  macOS compatibility.
- Run stable-toolchain locked build, formatting, Clippy with warnings denied,
  and all-target tests as implementation checks. Report checks actually run and
  unverified behavior; do not present the proposed validation as completed work.

### Measurement and acceptance record

Record reproducible commands and raw results alongside a concise pass/fail
summary. Declare hardware, macOS and terminal versions, Rust/build profile,
native tool versions, viewport sizes, fixture configuration, sample counts,
duration, instrumentation, and any measurement overhead.

| Proof | Required evidence and acceptance |
| --- | --- |
| Native launch and attach | Real Claude Code, then Codex CLI in an explicitly adopted checkout; same-session reattach, native prompt/approval and Ctrl-C behavior; no second managed active session in that worktree. |
| Navigation | Input event to visible selection feedback while sustained output and refresh run; report distribution and worst observed latency against the 100 ms target. Do not substitute event enqueue time for visible feedback. |
| Attach latency | Attach action to usable synchronized screen and input for an already-running session; report distribution and worst observed latency against the under-one-second target, excluding fresh agent startup. |
| Output/resource workload | Predeclare byte rate, total bytes, duration, escape/redraw mix, viewport changes, queue limits, subscriber lag, and storage delay. Run attached, dashboard-focused, and fully detached cases, plus slow subscriber and slow/full-storage cases. Report achieved output rate and any producer blocking. |
| Memory and queues | Measure dashboard and supervisor separately, child usage separately, peak/steady-state memory, queue occupancy/high-water marks, and screen/scrollback bounds. Run beyond queue saturation and multiple transcript chunks; demonstrate a bounded plateau rather than proportional growth with emitted history. Explain allocator/OS overhead and any exceeded budget. |
| Storage | Measure bytes emitted, captured and durably committed, transcript/metadata disk growth, persistence bytes/sec and flush interval distribution. Demonstrate the one-second healthy-storage target, successful clean-shutdown flush, and recovery of the durable prefix after abrupt failure. Record unflushed loss and failed writes honestly. |
| Fault responsiveness | During slow/full storage and subscriber lag, measure navigation/attach, bounded queues, visible failure and backpressure, recovery/stop, and retained output. Explain blocked producer behavior without claiming agent execution is suspended. |
| Reconnection | Record session and request identities and observable child counts through client loss, lost acknowledgement, and supervisor crash windows. No duplicate or automatic coding restarts; unresolved survivors block replacement. |
| Native status | Per tool/version matrix of scenario, actual PTY-compatible signal, visible result, and missing-evidence behavior; input, approval, completion, and interruption are required demonstrations. |
| Visual direction | Normal/small terminal screenshots compared with the image, including explicit explanations for native-screen presentation, one-session worktree limits, and unavailable reviews. |

Choose concrete resource caps and reproducible workload parameters during the
prototype, state them before measurement, and record the measured capacity and
limits. There is no established numeric memory/disk budget to invent as a
product promise. A missed latency or durability target, unbounded accumulation,
silent transcript loss, or native compatibility failure remains an open proof
failure with evidence and a proposed resolution, not a silently relaxed gate.

## Out of Scope

- Multiple registered projects, independent task branch/worktree creation, and
  the combined release workload of three projects, six sessions, ten PRs, and
  background review preparation.
- Full task/history search, archival management, safe worktree removal, and a
  complete native-resume workflow beyond this proof's recovery checks.
- Importing sessions launched outside the dashboard or transferring native
  conversations between tools.
- External editor/diff integration, staging/commit/push controls, PR feedback,
  merging, review discovery/preparation, and review publication.
- Full setup/configuration UI, macOS notifications, branding, exact image
  content, and detailed visual polish beyond a usable faithful composition.
- Automatic history retention/deletion, hosted state, multi-user control, and
  simultaneous controlling dashboards.
- Claims that this milestone satisfies every release coding criterion or that
  Linux fixtures establish native macOS support.

## Further Notes

- This spec implements the Milestone 1 proof described in the product,
  architecture, and milestone documents. The existing foundation has worktree
  discovery/navigation and terminal restoration, with no supervisor, sessions,
  adapters, or persistence implemented at specification time.
- The approved test seam was confirmed by the user before publication. Native,
  resource and recovery proofs have since been executed; see the
  [integrated evidence](../../docs/proofs/18-milestone-evidence.md). Ticket 18
  records the corrected native foreground-subprocess Stop failure and passing
  actual Stop/quit retests. Final source checks and resource revalidation remain
  open; this spec is not yet marked complete.
- Preserve accepted ADRs 0001 (supervisor ownership), 0003 (status evidence),
  0004 (history and explicit cleanup), and 0005 (uncertain launch reconciliation).
  Resolve feasibility problems explicitly before contradicting those decisions.
- Reference UI: [dashboard image](../../docs/images/conductor-dashboard-v3.jpg).
  Sources: [vision](../../VISION.md), [product requirements](../../PRODUCT.md),
  [architecture](../../docs/architecture.md), and
  [milestones](../../docs/milestones.md).
- Implementation must update architecture and user instructions to describe
  working launch, focus escape, detach/quit, recovery, history, and tested
  compatibility. Do not describe prototype assumptions as established support.
