---
name: rat-worktrees
description: Implement repository registration, task worktree creation or cleanup, Git discovery, task diffs, or editor diff integration in this project. Applies to product behavior rather than a developer's own checkout setup.
---

# Projects, worktrees and diffs

Paths are repository-relative.

1. Identify the affected C1–C7 behavior in [PRODUCT.md](../../../PRODUCT.md).
   Inspect `src/git.rs`, App selection, and supervisor adoption as relevant;
   distinguish existing behavior from the planned feature using the milestones.
2. Implement through the appropriate worker/owner. Preserve Unix path bytes
   and NUL-delimited Git records; display strings are not path round trips.
   Select the branch below for mutation or diff semantics.
3. Verify using real temporary repositories (`src/git.rs` and
   `tests/ownership.rs` supply patterns). Assert the intended Git result and
   preservation of unrelated work; report covered states and failure paths.

## Branches

- **Checkout ownership:** when changing path resolution, Git/file execution
  placement, worktree identity or diff delivery, read the
  [execution-machine boundary](../../../docs/architecture.md#execution-machine-boundary-direction-local-execution-today).
- **Discovery/selection:** extend the discovery/parser or App refresh tests for
  affected repository states. Verify refresh failure retains prior data and
  selection follows checkout identity rather than list position.
- **Task creation/adoption/removal:** read the projects/tasks lifecycle and C1–C7
  in [PRODUCT.md](../../../PRODUCT.md). Define per-repository serialization,
  cancellation, partial-success recovery and repeat-request behavior. Use
  `$rat-sessions` for execution reservations and cleanup evidence; a stopped UI
  label is insufficient. Before removal, resolve dirty/untracked and unpushed
  work, treating missing upstream evidence as uncertainty. Recheck identity and
  blockers immediately before mutation and preserve Git's refusal if external
  changes invalidate checks. Use `$rat-persistence` for durable task state.
- **Comparisons:** read the keyboard/diff requirements in
  [PRODUCT.md](../../../PRODUCT.md) before choosing Git commands. Name Working,
  WholeTask or Review in the model. Ordinary `git diff` omits untracked files;
  present them without altering the user's index. Exercise overlapping staged,
  unstaged and committed changes plus affected rename/deletion/binary cases.
  Use `$rat-github` for revision-pinned review comparisons.
- **Editor:** use `$rat-tui` for terminal handoff and restored dashboard context;
  pass editor argv and paths without shell interpolation. Native-editor support
  needs an actual configured-editor return/failure check on macOS.

For Git format/comparison questions, use installed command help and the
[worktree](https://git-scm.com/docs/git-worktree),
[status](https://git-scm.com/docs/git-status) or
[diff](https://git-scm.com/docs/git-diff) manual for the affected command.
