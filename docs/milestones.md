# Implementation milestones

The [product specification](../PRODUCT.md) defines the first release. These are
engineering checkpoints within that release, not a reduction of its requirements.

The [dashboard image](images/conductor-dashboard-v3.jpg) is the UI target throughout
implementation. Shape each usable UI slice toward its composition and visual
direction, showing real functionality and honest empty/unavailable states as
features arrive. Compare terminal screenshots against it at usable milestone
checkpoints and check smaller terminal sizes. Raise feasibility problems and
material deviations explicitly, with consequences and proposed alternatives.
The product specification identifies where the image differs from release
behavior, including worktree concurrency and native CLI presentation.

## Foundation — this change

- One Rust package with modules for terminal ownership, rendering, state/events,
  and background Git work.
- Run inside a repository or pass its path; discover real worktrees, navigate
  them, inspect branch/path metadata, and refresh without blocking navigation.
- Show loading and failure states, retain the last successful list on failure,
  and restore the terminal on normal exit, errors, and Rust panics.
- Document the architecture, contributor commands, and next risk investigation.
  Run formatting, Clippy, build, and tests locally before enabling CI.

This checkpoint does not yet launch or attach to agents.

## Milestone 1 — one usable coding session

Open the dashboard in one Git repository, list its worktrees, explicitly adopt
one, and launch or attach to one managed coding-agent session. Navigate the
dashboard while that agent runs. Begin with a PTY/supervisor prototype before
adding project management or review features. The first usable session UI should
already follow the target composition; detailed polish follows validated native
interaction.

Acceptance:

1. Discovery alone changes no repository files and launches no agents. Adoption
   identifies the chosen checkout. Refuse a second active session there.
2. Launch Claude Code in that checkout, attach, answer a real native prompt, and
   exercise native Ctrl-C. Repeat with Codex CLI before claiming both supported.
3. Resize repeatedly while attached and detached. The child receives the current
   terminal size, its screen is coherent, and keyboard input arrives correctly.
4. A documented escape chord returns to the dashboard without stopping the
   agent. Reattach to the same session without starting a duplicate.
5. Navigate and refresh while a fixture streams sustained output. Measure input
   to visible navigation feedback against the product's 100 ms target; measure
   attach against its one-second target. Record hardware, workload, and versions.
6. Kill the dashboard, reopen it, and reconnect to the surviving supervisor and
   session. Ordinary quit explicitly stops managed activity; detach preserves it.
   Supervisor failure restores records as interrupted without restarting coding.
   Lose a launch acknowledgement and verify reconnect recovers the same request's
   result. Crash around process creation and reconcile any surviving child before
   allowing a replacement launch.
7. Capture readable history independently of native resume. Test exit, missing
   executable, spawn failure, interrupt, and transcript recovery on macOS.
   Measure the one-second healthy-storage flush target, clean-shutdown flushing,
   and recovery of durable output after abrupt failure.
8. For each supported native CLI version, demonstrate requests for input and
   approval, turn completion, and interruption using tool evidence. Verify that
   missing evidence produces an unknown state rather than an inferred outcome;
   see [the status decision](adr/0003-evidence-based-agent-status.md).
9. Measure bounded memory and queue behavior, transcript disk growth, and
   persistence throughput under sustained output, including detached sessions
   and slow subscribers. Exercise slow/full storage without silent output loss
   or a frozen dashboard, and verify explicit history cleanup; see [the storage
   decision](adr/0004-preserve-history-with-explicit-cleanup.md).

The [integrated evidence record](proofs/18-milestone-evidence.md) tracks the
versioned native proofs, resource measurements, visual comparison and final
checks. The actual native foreground-tool Stop failure was corrected and native
Stop/quit retests passed. Final source checks and live-tracker resource
revalidation passed on macOS; this scoped milestone proof is complete for the
recorded Claude Code 2.1.318 / Codex CLI 0.167.0 versions and declared workload.

These checks resolve the highest-risk boundary; they do not claim the complete
release workload or all coding acceptance criteria C1–C9 are satisfied. New-task
worktree creation, multiple projects, history search, editor/diff integration,
own-PR feedback/merging, setup, and notifications follow this proof.

## Milestone 2 — coding coordination

Following the native-session proof, complete multi-project navigation, independent
task and worktree creation, task/history search, native resume, archival and safe
worktree removal, and working/whole-task diffs with external-editor return.

Acceptance: complete C1–C7, including real macOS checks with both supported tools.
Persistence and necessary setup arrive with the first feature that needs them;
they are not deferred to release validation.

Tickets 01–18 implement the local coordination features. Ticket 19 records a
clean frozen all-target aggregate, the three-project/six-stream measurement and
the integrated six-native-session workflow, including installed editors and
local archive/removal recovery. Final integration review/checks and real
sleep/wake and physical reboot remain open. C6 and milestone completion remain
unclaimed; injected boot identities do not establish the physical gate. See the
[qualification record](proofs/m2-19-qualification.md) and the explicitly unexecuted
[physical-transition plan](proofs/m2-19-physical-transition-plan.md).

Begin with the prerequisites identified from the completed session proof:
retained project/workspace/session records and migration, explicitly targeted
coding operations with canonical-worktree exclusivity, fair multi-session capture
and search scheduling, and execution-owned bounded Git/setup jobs with recoverable
outcomes. Qualify the native creation/naming/history experience early rather than
assuming terminal recordings are structured chat messages.

Preserve the [later-milestone boundaries](architecture.md#compatibility-with-later-milestones):
restricted revision-pinned reviewers, structured review drafts and GitHub actions
remain Milestones 3–4; macOS notifications and combined release validation remain
Milestone 5. Coding-history projection and local archive recovery must not replace
review records or future PR identity. These are early Milestone 2 tickets, not a
new prerequisite milestone or a reopening of the completed Milestone 1 proof.

## Milestone 3 — GitHub review preparation and human submission

A separate milestone in the same first release, following the session proof:

- Discover personal requests via existing GitHub CLI authentication. Prepare one
  review at a time with the configured reviewer, with visible queue control and
  failures. Explain usage costs and the automation default during setup.
- Save local drafts against the examined revision, preserve human edits, and
  keep suggestions undecided until explicitly selected. Do not run review
  builds or tests automatically.
- Show drafts beside revision-specific diffs, support editing and explicit
  revalidation of stale comments, and preview exact selected content/outcome.
  Record both head and base/comparison revisions; exercise base-only comparison
  changes as well as new author commits.
- Publish only through explicit user submission. Refresh eligibility/revision
  and reconcile uncertain network outcomes before allowing a duplicate retry.
- Verify product criteria R1–R11, including native reviewer, GitHub, editor,
  interruption, and uncertain-publication checks.

## Milestone 4 — own-PR continuation

Link task PRs, show incoming feedback, explicitly send selected feedback to the
coding agent, and provide explicit merge and per-PR auto-merge actions.

Acceptance: complete C8–C9, including blocked merges, auto-merge cancellation,
uncertain remote outcomes, and preservation of task/worktree history.

## Milestone 5 — combined release validation

Complete remaining setup and notification behavior, then exercise both workflows
together against every product acceptance criterion. Validate the reference
workload of three projects, six coding sessions, and ten tracked PRs with one
review being prepared; measure navigation, attach, and remote-discovery targets.
Verify offline use, sleep/wake, dashboard loss, reboot, and recovery on macOS.

Acceptance: C1–C9, R1–R11, setup, notifications, interruption behavior, and the
combined responsiveness requirements all pass with recorded native-tool versions
and workload/hardware details. Earlier checkpoints alone do not establish release
readiness.
