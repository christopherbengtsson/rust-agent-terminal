# Milestone 2 ticket 14: Whole workspace inspection

2026-11-06, macOS 26.2, arm64, stable rustc 1.99.1, Git 2.50.0.
Isolated worktree `/private/tmp/rat-m2-14`, branch `m2-14-whole-workspace`,
started from integration `5c1a6044b6d2c0b53f55df6f6f04f9008ec37c1b`.
The initial implementation is `7da13a7`.

## Behavior and ownership

`f` always opens Working. Within inspection, `w` explicitly switches to Whole
workspace; `b` enters a full local ref (`refs/heads/...` or
`refs/remotes/...`). Enter saves and compares; Esc cancels entry. Adopted
workspaces with no selected base receive selection guidance. No inferred current
branch, archive recovery ref, remote fetch or silent fallback supplies a base.

Whole workspace resolves the selected local ref to its current commit, resolves
HEAD and requires exactly one common ancestor. It combines the files changed by
branch commits since that ancestor with current staged, unstaged and new files.
The selected patch compares the common ancestor to current working files;
separate HEAD/index and index/files sections retain staged and unstaged detail.
Thus a commit does not hide earlier work, and staged content remains visible when
a subsequent unstaged edit replaces or undoes it. Conflicts show an unresolved
label and current markers. Binary, rename/deletion, new, oversized and submodule
cases retain bounded explanations; nested submodule content remains outside the
comparison.

The snapshot names captured branch, HEAD, actual local base tip and common
ancestor. The selected ref and original selection-time commit are durable,
independent of the current branch and workspace creation's original facts.
Externally moved refs recompute against the new local tip and label the old
selection commit. A branch switch is visible in the next snapshot's branch/HEAD.
Missing refs, unrelated histories and criss-cross histories with multiple common
ancestors require explanation/selection. No inspection fetch runs.

Base resolution and file/Git access execute in the existing cancellable Git
worker. Selecting a base queues an attributed sole-writer commit; the successful
snapshot is published only after that operation commits. ENOSPC or queue failure
returns an error and retains the previous durable choice. Shutdown waits for the
pending commit. The additive `workspace_comparison_bases` table uses raw ref bytes
and `base_commit`; it does not allocate schema 6 or change unrelated history
records. Ticket 09 can seed the initial choice with INSERT OR IGNORE from original
creation metadata, preserving later choices and immutable original-base facts.

Each Git command retains the inherited two-second/256 KiB cap and owned-group
cleanup. Admission remains one comparison worker; list/content limits, NUL path
records, literal pathspecs, no optional locks, disabled fsmonitor/external diff/
textconv, and non-regular new-file refusal remain Working's boundaries. Status,
HEAD, base and branch rechecks can warn of observed movement during capture; a
matching recheck does not establish atomic file contents.

Wire comparison protocol 2 guards the Whole fields. The new dashboard refuses a
live protocol-1 owner before sending unsupported requests, since that owner could
otherwise ignore the new fields and silently return Working. Pinned PR Review
head/base records and editor artifacts remain separate later work.

## Verification completed so far

The approved seams are real temporary Git repositories, the App/dashboard and
real Controller interface. The first committed-plus-current tracer failed to
compile because Whole inspection did not exist, then passed after implementation.
Supporting checks exercise actual commits and Git results rather than mocked
command routing.

Passed on the initial implementation source:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --bin agent-terminal working_diff::tests -- --nocapture
cargo test --locked --bin agent-terminal app::tests
cargo test --locked --bin agent-terminal ui::tests
cargo test --locked --test ownership whole_workspace_base_selection -- --nocapture
cargo test --locked --test ownership working_diff_jobs_are -- --nocapture
```

Ten diff tests include: previous commits plus overlapping staged/unstaged/new
files; exact index-byte preservation; committed and current renames/deletions/
binary changes; real merge conflicts; full-ref selection; external branch/ref
movement; missing refs; unselected or rejected ambiguous short/archive refs;
unrelated histories; and two actual merge bases in a criss-cross graph.
Repository-configured diff, textconv and fsmonitor programs remain unexecuted in
both comparisons. Raw newline paths survive selection and file lookup. The host
APFS rejected an attempted invalid UTF-8 filename with EILSEQ; this macOS run
therefore qualifies newline/control bytes, with an additional non-UTF-8 path case
compiled for Unix filesystems that support it. No non-UTF-8 APFS filename claim is
made.

The real Controller test starts a private supervisor without a coding child. It
checks missing-base guidance, injected registry ENOSPC with no optimistic saved
choice, successful attributed base persistence, exact compared HEAD, no implicit
launch, clean restart, restoration of the original selected commit, and a moved
base producing a labelled new comparison. It passed in 1.27 seconds. The existing
Working worker bounds/cancellation test passed in 3.51 seconds. These are test
observations, not workload latency guarantees.

Twenty-one App and thirteen UI regression checks pass. The new App/dashboard
interaction check renders at 160×40 and 60×18, selects a local base explicitly,
inspects committed content through scrolling, and reopens Working. It preserves
the existing reference composition and compact content rows. Actual dashboard
PTY and protocol-owner checks are implemented but await the shared PTY slot;
no new native-session, editor, streaming or release-workload claim is made here.
