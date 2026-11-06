# Milestone 2 ticket 13: Working inspection

2026-11-06, macOS 26.2 (25C56), arm64, stable rustc 1.99.1, Git 2.50.0.
One Rust package; isolated checkout `/private/tmp/rat-ticket13`, branch
`feature/m2-ticket13`, based on integration `b74faa31`. Final qualification also
includes concurrent-session owner `9a1dd26` and integration `610c723` (ticket 08).
Actual PTY captures were made on the ticket 13 runtime source at `0fccb2d`, with
the capture test/cells committed in `8b347b0`. Subsequent integration adds readable
history; its App/UI and Working seams were rerun below.

## Behavior and ownership

`f` opens Working inspection. In the retained workspace picker it targets the
selected stable workspace ID; in the worktree list it resolves the selected raw
checkout path against retained canonical checkout facts and refuses unknown
workspaces. Inspection does not adopt, launch or select another coding runtime.
Every open creates a fresh Working request and resets the prior file/comparison.
`j`/`k` selects a file; arrows scroll lines/columns, Page Up/Down moves eight lines,
and Home returns to the start. `r` refreshes; `f`/Esc returns. Stop, quit, detach
and storage retry remain available.

The supervisor owns one comparison job, resolves the stable workspace through the
registry, and verifies its canonical checkout identity. The request remains global
and workspace-owned after the ticket 06 merge: putting it inside a runtime-targeted
envelope is refused. A targeted Stop also cancels the comparison job. Git work,
canonicalization and file reading never run in App or rendering. The existing
process-group runner bounds each Git command to two seconds/256 KiB, drains pipes
without blocking controls, cancels and reaps its child/helpers, and limits worker
admission to one job. Busy, timeout and cancelled outcomes carry the exact
workspace/request identity. No mutation lease is taken: comparison never mutates
Git's index or refs.

Porcelain v1 `-z --untracked-files=all --ignore-submodules=none` preserves Unix path
bytes, including newline/non-printing names. File ordering uses those bytes.
Tracked edits have separate Staged (HEAD/index) and Unstaged (index/files) sections;
one file may have both. New file content is read without staging. Renames retain
both names; deletions have patches; binary changes have Git's binary label.
Unmerged indexes identify conflicts and show the current file/conflict markers
without pretending that is a resolved comparison. Submodules explicitly exclude
nested file inspection. Missing/unsupported/symlink/non-regular reads and
oversized patches/files get explanations. No optional index locking, configured
external diff, textconv or fsmonitor program runs during comparison.

Presentation caps are 1,024 files, 64 KiB per selected patch/new file, bounded Git
output, and the existing bounded transport. Excess file count is explicit; excess
status output makes the comparison unavailable with its bound error. Terminal and
bidi controls are escaped. New file opens use NOFOLLOW/NONBLOCK and refuse
non-regular content. Snapshot workspace, canonical checkout, request and capture
time are visible. Snapshots say “not live”; observed status changes during capture
have an additional warning. Matching pre/post status does not claim atomic file
contents. Refresh preserves the selected raw path and a surviving context line;
old workspace/request replies cannot replace a newer view. Failed refresh retains
its previous snapshot with a failure label. Reconnect opens a new inspection;
there is no retained live comparison assumption.

## Evidence and checks

The agreed TDD seams were actual temporary Git repositories, the public
App/dashboard interaction, and the real Controller protocol. The initial real-Git
tracer test failed to compile before implementation; the UI tracer then failed
because no Working content was rendered. Both passed after the corresponding
vertical implementation. Expanding the actual PTY fixture to tracked and new
files exposed Git's tracked-first porcelain order; raw-byte path sorting made
navigation predictable. The first close/reopen fixture accidentally encoded Alt-f
by sending Esc and f together; it now waits for dashboard focus between those
separate actions. The fixture also waits for retained workspace metadata before
requesting a comparison after asynchronous adoption.

Final scoped checks passed:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --bin agent-terminal working_diff::tests
cargo test --locked --bin agent-terminal app::tests
cargo test --locked --bin agent-terminal ui::tests
cargo test --locked --test ownership working_diff_jobs_are -- --nocapture
```

Five Working tests cover actual Git repositories: overlapping staged/unstaged
changes; untracked newline paths and escaped controls; exact index-byte
preservation; refused new symlinks; staged renames, deletions, binary content,
64 KiB content limits and a real merge conflict; configured external diff,
textconv and fsmonitor executables remaining unexecuted; dirty real submodule
labelling; 1,030 new files producing 1,024 entries plus six explicit omissions;
and normal/small dashboard rendering, selection, context, stale workspace/request
rejection, refresh after external writes, default reopening and Stop availability.
The 21 App and 13 UI tests passed after both integration merges.

The real Controller test launches only an isolated supervisor, never a coding
child. It verifies workspace/request attribution, a new file's content, no
implicit launch, refusal of a runtime-wrapped comparison, one-job saturation,
responsive Screen during slow Git, targeted Stop cancellation, a two-second Git
timeout, explicit retry and clean shutdown. The slow wrapper actually sleeps ten
seconds before Git; cancellation and timeout kill that owned group instead of
waiting for its natural exit. This final test passed in 3.30 seconds; durations
are test observations, not workload latency guarantees.

Actual dashboard proof:

```sh
RAT_DIFF_EVIDENCE=/private/tmp/rat-ticket13-evidence \
  cargo test --locked --test ownership \
  working_diff_dashboard_inspects -- --nocapture
swift scripts/render-terminal-capture.swift \
  /private/tmp/rat-ticket13-evidence/m2-13-working-pty-160.json
swift scripts/render-terminal-capture.swift \
  /private/tmp/rat-ticket13-evidence/m2-13-working-pty-60.json
```

This passed at 160×40 and 60×18 in 2.81 seconds after ticket 06 integration.
It adopts a real repository, opens Working, changes files while the dashboard is
open, refreshes the same selected new file, navigates to overlapping tracked edits,
scrolls between Staged/Unstaged sections, closes and reopens with the default file,
and quits normally. Every scenario uses a private state directory and cleans up
its owned supervisor. No native tool or streaming fixture was launched. The
concurrent file writes are controlled by the test, representing the shared-file
behavior of an active agent; this is not a native Claude/Codex diff integration
claim. The later full 06/08 integration gate owned the PTY slot, so no conflicting
broad/native suite ran from this worktree.

Raw captures and inspected rasterizations:

- [Normal cells](screens/m2-13-working-pty-160.json),
  [text](screens/m2-13-working-pty-160.txt),
  [PNG](screens/m2-13-working-pty-160.png).
- [Small cells](screens/m2-13-working-pty-60.json),
  [text](screens/m2-13-working-pty-60.txt),
  [PNG](screens/m2-13-working-pty-60.png).

The normal view retains the vision's reviews/project tree beside a prominent
inspection pane and visible status/help. Working temporarily occupies the native
session pane. The compact view intentionally hides the sidebar, uses a vertical
file/content layout and still displays full workspace/request IDs, checkout,
freshness, selected state and controls. Reviews remain visibly unavailable;
there are no placeholder comparison/editor controls. Inherited NO_COLOR was
respected, so these captures establish content, geometry and interaction rather
than native-terminal color compatibility. PNGs rasterize parsed real PTY cells;
they are not Terminal.app/iTerm screenshots.

Whole workspace/base comparison is ticket 14; external-editor handoff is ticket
15. Pinned PR Review comparisons remain a later milestone. No native compatibility,
atomic live filesystem snapshot, unbounded repository coverage, or full release
workload/resource claim is made here. The parent integration owns the final
all-target gate; this scoped ticket did not run unfiltered terminal-restoration
or broad streaming suites concurrently with other native checks.
