# Ticket 16 — Stop, save and archive

2026-11-07, macOS 26.2 (25C56), stable rustc/cargo 1.99.1, Git 2.50.0.
Implementation and tests are in `src/archive.rs`, `src/archive_git.rs`,
`tests/support/archive_git.rs` and the seven `archive_*` Controller/dashboard
scenarios in `tests/ownership.rs`. This is scoped archive evidence, not full C6
or combined native/resource/reboot qualification; ticket 19 retains that work.

## Working behavior

`K` previews the selected workspace; Enter explicitly stops/saves/archives.
`w`, then `z`, includes archived workspaces; `K` and Enter unarchive visibility
without starting coding. `z` cancels the exact archive request, Escape dismisses
without cancelling, `j/k` scroll receipts/exclusions, `q` stops/quit and `d`
detaches. Enter retries the same operation. If external work changed the prepared
snapshot, cancel it, press `n` and explicitly confirm a new snapshot.

The worker waits for every relevant canonical session owner and setup attempt,
including pending native resume cancellation, final capture and attributed durable
writes. Failed/uncertain cleanup blocks saving. A whole-repository mutation lease
exists only inside bounded Git work, never while waiting for Stop or SQLite.

Dirty work becomes a clearly named **Archive checkpoint**, using normal Git
identity, hooks and signing. Clean work keeps its HEAD without an empty commit.
The receipt says **saved locally, not pushed**, shows the commit/ref, and lists
ignored paths outside coverage. Conflicts, submodules/nested repositories,
split/sparse/assume-unchanged/skip-worktree/intent-to-add state, observed external
edits, hook/identity errors and uncertain checks leave the workspace unarchived.
Folders, ignored files, chats, workspace/native identities and creation provenance
remain independent of code refs and archive visibility.

## Index and recovery strategy

The read-only exploration in `/tmp/rat-m2-archive-experiment.json` established that
an ordinary alternate-index commit can lose staged-only versions from the saved
checkpoint and make an unchanged real index falsely dirty against new HEAD.
The implementation does not rely on that incomplete strategy.

While owning the real index lock, preparation records the real index byte hash,
original staged tree, working snapshot, symbolic branch and parent. Immutable
alternate index artifacts remain under the Git directory. Separate workspace refs
pin original staged (`-index`) and prepared (`-prepared`) trees. Ordinary
`git commit` runs on a separate commit index so hook mutation cannot rewrite the
immutable prepared snapshot. Only a matching tree/parent/operation, unchanged
canonical checkout/branch/HEAD and working snapshot, and unchanged real index
permit atomic replacement of the owned index lock with the saved HEAD index.
Staging is combined on success per ADR 0007, and staged-only content remains
readable with `git show '<receipt-ref>-index:path'`. Failures do not restore an old
index over external work. A successful Git save followed by a metadata failure
already has the combined index; the original staged tree remains protected.

Schema 10 stores Waiting/Prepared receipts before their dependent phases.
Prepared facts cannot change under the same request ID. Saved HEAD is pinned at
`refs/rat/archive/<workspace>/<request>` before the SQLite transaction records
both the exact receipt and archived visibility. Matching durable completion is
required for acknowledgement. Restart loads incomplete operations without
repeating Git or coding; explicit retry recognizes a prior commit by its parent,
tree and operation-bound message together. Clean and dirty replay tests create no
duplicate commit. Transcript deletion never removes these refs or records.

## Demonstrated checks

Public Git Job tests cover:

- staged-only content overwritten in working files, ordinary partial staging,
  new/ignored files, binary data, raw newline/tab filenames and deletions;
- clean HEAD, clean empty-tree HEAD with no real index, staged-only work when
  working files equal HEAD, and clean/dirty lost-reply replay;
- rejecting pre-commit hooks which mutate their alternate index, post-commit
  working edits and operation-message alteration, missing author identity,
  conflicts, nested repositories, split/sparse and intent-to-add refusal;
- external file/staging edits, owned cancellation after Git advanced HEAD,
  unchanged original real index on Git failure, recovery refs surviving reflog
  expiration and `git gc --prune=now` after visible branch/index artifacts vanish.

Seven actual Controller/dashboard/supervisor scenarios pass together:

1. A live scripted PTY session has a capture tail when archive begins. Owned Stop
   settles capture, saves working/new files, protects staged-only content, leaves
   a clean real index, retains ignored files, and replays a lost acknowledgement.
   Start is refused while archived. Transcript deletion and GC preserve saved
   code; unarchive keeps the original stopped managed session identity.
2. Injected `archive-result` SQLite failure happens after Git saved and pinned
   HEAD. The workspace stays visible; cancellation of the last durable intent
   works, and retry stores that same checkpoint once.
3. Real outer dashboard PTYs at 160×40 and 60×18 exercise `K`, Enter, receipt,
   unarchive and quit. The receipt is visible without scrolling at both sizes.
4. Archive stops an owned setup, settles its output and disarms Create and start.
   Its native marker never appears; another live coding session accepts input
   afterward. Retrying setup while archived is refused.
5. An otherwise valid native-resume lookup is stalled, then archive begins.
   HEAD cannot move before lookup cancellation settles. Releasing the lookup
   never starts its fake Claude child. Restart restores receipt/visibility and
   refuses another resume without launching coding.
6. A restored setup remains uncertain despite fixture proof its direct process
   exited. Archive cannot mutate HEAD or claim success, and exact cancellation
   leaves it unarchived. Cleanup uses explicitly labelled debug boot evidence;
   no physical reboot is claimed.
7. The supervisor is killed after Git completed and while archive-result SQLite
   storage is stalled. Restart exposes durable Prepared/unarchived state without
   automatic work. Cancelling that restored intent and explicitly retrying the
   same request stores the existing commit; the
   branch still has exactly base + one checkpoint.

Scoped validation commands:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --bin agent-terminal archive_git::tests
cargo test --locked --test ownership archive_ -- --nocapture
cargo test --locked --test ownership selected_resume_uses_original_segment_tool_and_native_identity -- --exact
cargo test --locked --bin agent-terminal app::tests
cargo test --locked --bin agent-terminal ui::tests
cargo test --locked --bin agent-terminal persistence::concurrent_tests::selected_resume_choices
```

The final complete archive scenario run passed 7 tests in 16.64 seconds. The
selected-resume ownership regression passed in 4.35 seconds; App 32, UI 18 and
selected-resume persistence 1 passed. All 15 Git Job tests passed again in 2.14 seconds after explicitly hardening
Git reference writes; locked build, formatting and warnings-denied all-target
Clippy passed. Product/test source is commit
`ab268c9` (code implementation `6e454e1`, latest integration merged at `8f0ffe2`).
The configuration-only reference-hardening follow-up leaves the seven Controller
flows unchanged and was verified through the 15 real-Git Job tests and Clippy. An initial `persistence::tests` filter matched zero tests;
it is not counted as storage evidence. Final all-target runtime/native suites
belong to combined ticket 19 qualification, not this scoped run.

## Layout evidence and limits

Actual parsed outer-PTY captures:
[normal preview](screens/m2-16/archive-160-preview.txt),
[normal receipt](screens/m2-16/archive-160-receipt.txt),
[small preview](screens/m2-16/archive-60-preview.txt),
[small receipt](screens/m2-16/archive-60-receipt.txt).
The normal layout retains reviews above the project/worktree sidebar with the
archive panel in the prominent session area. The existing compact layout hides
the sidebar; receipt-first text and scrolling keep actions and save coverage
usable. These captures were compared with the vision image's composition; they
are parsed terminal captures, not graphical Terminal.app screenshots. Later
review UI remains unavailable as already documented.

Git commands explicitly request `core.fsync=committed,reference` and fsync method; index
artifacts/directories are synced and SQLite retains FULL/fullfsync. This does not
establish physical power-loss, reboot or filesystem/hardware guarantees, and the
two stores are not atomically committed together. An abrupt supervisor loss
*during* Git can leave real/Git locks; the app refuses uncertain locks rather than
automatically deleting them. Verify no Git process owns one before manual repair.
External actors cannot be locked out of working files after the final snapshot
check; a receipt identifies the saved snapshot, not future external edits.
Git filters/LFS retain ordinary Git semantics rather than backing up arbitrary
external resources. No branch deletion/restoration or folder removal is claimed
here (tickets 17/18). No real Claude/Codex archive compatibility, combined workload,
sleep/wake or physical reboot was rerun in ticket 16; ticket 12 native evidence
is prior integration evidence, and ticket 19 owns final combined qualification.

The [Git configuration reference](https://git-scm.com/docs/git-config/2.48.1)
distinguishes committed object hardening from the separate `reference` component;
both are requested explicitly.

## Integration merge verification

Ticket 16 was merged with `--no-ff` into
`integration/milestone-2-coding-coordination` at
`f6876d8536292a30e9c27cf49849b4fd3e62d75b`, from completed branch
`ticket16-archive` at `34533ab223fe339dcc518d6ff638e378825042d4`.
Blocker ticket 10 and ticket 16 both had `Resolved: yes`. The integration parent
`3b4b364456a115c3b357331390e906c1915af134` was already an ancestor of the
completed branch. The merge was conflict-free and its tree exactly matched that
branch; no product or test edits were made during integration.

On this merged tree, locked build, formatting check, all-target warnings-denied
Clippy and `git diff --check` passed. The focused commands listed above passed:
15 real-Git archive Job tests (2.15 seconds), 32 App tests, 18 UI tests and the
selected-resume persistence test (0.42 seconds). The seven prior Controller/PTY
archive scenarios and selected-resume ownership check were not rerun for this
unchanged merge tree; their source/version scope remains as recorded above.
The full suite and combined native/resource/reboot qualification remain ticket
19 work. Native compatibility, physical power-loss and stale Git lock/manual
repair limitations remain unchanged.

The clean, fully merged ticket worktree was removed after ancestry and clean-state
verification; its `ticket16-archive` branch was retained.
