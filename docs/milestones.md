# Implementation milestones

The [product specification](../PRODUCT.md) defines the first release. These are
engineering checkpoints within that release, not a reduction of its requirements.

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
adding project management, review features, or visual polish.

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
7. Capture readable history independently of native resume. Test exit, missing
   executable, spawn failure, interrupt, and transcript recovery on macOS.

These checks resolve the highest-risk boundary; they do not claim the complete
release workload or all coding acceptance criteria C1–C9 are satisfied. New-task
worktree creation, multiple projects, history search, editor/diff integration,
own-PR feedback/merging, setup, and notifications follow this proof.

## Milestone 2 — GitHub review preparation and human submission

A separate milestone in the same first release, following the session proof:

- Discover personal requests via existing GitHub CLI authentication. Prepare one
  review at a time with the configured reviewer, with visible queue control and
  failures. Explain usage costs and the automation default during setup.
- Save local drafts against the examined revision, preserve human edits, and
  keep suggestions undecided until explicitly selected. Do not run review
  builds or tests automatically.
- Show drafts beside revision-specific diffs, support editing and explicit
  revalidation of stale comments, and preview exact selected content/outcome.
- Publish only through explicit user submission. Refresh eligibility/revision
  and reconcile uncertain network outcomes before allowing a duplicate retry.
- Verify product criteria R1–R11 and the full combined responsiveness workload,
  including real macOS tool/editor/recovery checks, before calling the release
  usable. Complete the remaining coding and own-PR criteria as well.
