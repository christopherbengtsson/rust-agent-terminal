# Milestone 2 ticket 14: Whole workspace inspection

2026-11-06, macOS 26.2, arm64, stable rustc 1.99.1, Git 2.50.0.
Isolated worktree `/private/tmp/rat-m2-14`, branch `m2-14-whole-workspace`,
started from integration `5c1a6044b6d2c0b53f55df6f6f04f9008ec37c1b`.
The initial implementation is `7da13a7`. Final runtime source includes corrected
compatibility checkpoint `c20cd49` through merge `8dfb046` and the bounded Git
configuration correction `ce32fdf`. The passing dashboard/protocol test source is
`e5c1341` (only a title-assertion correction follows `fdbf66a`; runtime unchanged).
Before completion, integration was rechecked at `5c1a604` and merging it reported
already up to date; corrected `c20cd49` is additionally included.

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

## Verification

The approved seams are real temporary Git repositories, the App/dashboard and
real Controller interface. The first committed-plus-current tracer failed to
compile because Whole inspection did not exist, then passed after implementation.
Supporting checks exercise actual commits and Git results rather than mocked
command routing.

Passed on the initial implementation and repeated after corrected integration:

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
base producing a labelled new comparison. It passed in 1.27 seconds, then passed again in 1.38 seconds after the
compatibility correction. The existing Working worker bounds/cancellation test
passed in 3.51 seconds before that correction. These are test
observations, not workload latency guarantees.

Twenty-one App and thirteen UI regression checks pass. The new App/dashboard
interaction check renders at 160×40 and 60×18, selects a local base explicitly,
inspects committed content through scrolling, and reopens Working. It preserves
the existing reference composition and compact content rows. The final build,
format check, all-target warnings-denied Clippy, ten diff tests, twenty-one App
tests and thirteen UI tests all passed again on `e5c1341`.

## Actual terminal interaction and visual review

After ticket 09 released the shared terminal slot, these scoped checks passed:

```sh
RAT_DIFF_EVIDENCE=/private/tmp/rat-m2-14-evidence \
  cargo test --locked --test ownership \
  whole_workspace_dashboard_switches -- --nocapture
cargo test --locked --test ownership \
  dashboard_refuses_legacy_live_owner -- --nocapture
```

The Whole dashboard test passed in 4.68 seconds. It uses an actual outer PTY at
160×40 and 60×18, adopts a real repository, explicitly switches comparisons,
selects a base, inspects committed plus overlapping staged/unstaged and new files,
refreshes after controlled current-file edits, moves the local base externally,
checks the movement label, and closes/reopens into Working. All private owners
stop normally; no coding child is launched. The first attempt failed only because
the reopen assertion matched the available-switch footer's “Whole workspace” text;
restricting that assertion to the bordered comparison title corrected the harness.
No runtime change was needed for the passing rerun.

The legacy-owner test passed in 12.61 seconds. Its added case advertises all other
current capabilities with comparison protocol 1. The dashboard refuses that live
owner, sends zero unsupported request bytes, detaches locally, and leaves the
locked owner's database unmigrated. The test also retains older missing/legacy
Git-protocol cases.

Raw cells, text and inspected rasterizations:

- Normal identity/base: [cells](screens/m2-14-whole-pty-160-header.json),
  [text](screens/m2-14-whole-pty-160-header.txt),
  [PNG](screens/m2-14-whole-pty-160-header.png).
- Normal overlapping edits: [cells](screens/m2-14-whole-pty-160-patch.json),
  [text](screens/m2-14-whole-pty-160-patch.txt),
  [PNG](screens/m2-14-whole-pty-160-patch.png).
- Normal moved base: [cells](screens/m2-14-whole-pty-160-moved.json),
  [text](screens/m2-14-whole-pty-160-moved.txt),
  [PNG](screens/m2-14-whole-pty-160-moved.png).
- Small identity: [cells](screens/m2-14-whole-pty-60-header.json),
  [text](screens/m2-14-whole-pty-60-header.txt),
  [PNG](screens/m2-14-whole-pty-60-header.png).
- Small overlapping edits: [cells](screens/m2-14-whole-pty-60-patch.json),
  [text](screens/m2-14-whole-pty-60-patch.txt),
  [PNG](screens/m2-14-whole-pty-60-patch.png).
- Small moved base: [cells](screens/m2-14-whole-pty-60-moved.json),
  [text](screens/m2-14-whole-pty-60-moved.txt),
  [PNG](screens/m2-14-whole-pty-60-moved.png).

All six PNGs were generated with `scripts/render-terminal-capture.swift` and
visually inspected against `docs/images/conductor-dashboard-v3.jpg`. The normal
view retains reviews above the project/worktree tree and a prominent inspection
pane, with status and actual keyboard actions below. Inspection occupies the
native session area temporarily. Reviews remain honestly unavailable. The compact
view intentionally hides the sidebar, retains workspace/request/checkout identity
and state, and uses three content rows plus vertical/horizontal scrolling.
Long comparison labels and hashes require scrolling at 60 columns; the small
captures show identity, patch and movement as separate contexts. This preserves
the inherited small-layout boundary rather than claiming simultaneous visibility
of every comparison field.

Inherited NO_COLOR was respected. These PNGs rasterize real parsed PTY cells;
they do not establish Terminal.app/iTerm rendering or native-tool compatibility.
The controlled filesystem edits represent shared-file activity without a native
agent. No editor handoff, new native-session, streaming performance or full
release-workload claim is made. The parent owns the final aggregate all-target
gate; no unfiltered terminal-restoration or native suite ran concurrently here.
Creation's original-base facts and initial INSERT OR IGNORE seed are coordinated
with ticket 09 and remain separately qualified by that feature's evidence.
