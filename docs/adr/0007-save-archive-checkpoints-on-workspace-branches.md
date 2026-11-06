# Preserve archive checkpoints as recoverable local commits

Accepted during the Milestone 2 interview on 2026-11-06. Explicit archiving saves
unfinished code changes as a clearly named local commit on the workspace branch,
following Conductor's documented behavior, rather than placing them in the
repository's shared stash list. This avoids a separate stash-application step,
at the cost of an unfinished-work commit in branch history and combining
staged/unstaged edits.

Refined later in the same interview: preserving every visible branch would
accumulate stale names, so archive recovery must retain the saved commit through
a workspace-specific Git reference independently of that branch. Confirm this
recovery point before deleting the local branch. Delete local branch on archive
defaults on for product-created branches and off for adopted existing branches;
the latter require explicit selection. Restore can recreate a branch, resolving
name collisions without overwriting another branch. Remote branches are never
automatically deleted. This adds archive-ref lifecycle/recovery work in exchange
for branch cleanup without discarding archived code.

Archiving requires a stopped session and remains separate from removing the
worktree or deleting history. Confirm saving before reporting archive success;
failures leave the workspace unarchived, and feedback distinguishes local saving
from pushing. A checkpoint is not a promise to capture ignored files or arbitrary
nested repository state; coverage and unsupported cases must be explicit.
