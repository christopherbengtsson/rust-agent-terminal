# Ticket 17 — recoverable archive branch cleanup and return

2026-11-07, macOS 26.2 (25C56), stable Rust 1.99.1. Public Git tests used
Homebrew Git 2.50.0; supervisor scenarios used `/usr/bin/git` 2.50.1 (Apple
Git-155), files refs. Started on integration commit `3edf1c0`; ticket 16's
`f6876d8` is an ancestor. This is scoped branch/archive evidence. Ticket 19 owns
combined native, resume, resource and sleep/wake/reboot qualification.

## Behavior and safety boundary

`K` previews archive, including **Delete local branch on archive**, exact target
and `b` toggle. Product-created branches default on; adopted branches default off.
Waiting for discovery does not invent an adopted deletion target. The target can
populate while preview remains open. Submitted policies and immutable creation
branch/base facts cannot change under the same operation request.

A successful local checkpoint/recovery reference and **durable archived receipt**
precede optional branch cleanup. Cleanup saves a separate result; a retained
branch does not undo confirmed code saving/archive. The bounded Git worker alone
holds the per-common-directory mutation lease. It revalidates checkout identity,
recovery, direct branch, saved tip, clean files/index, creation provenance,
main/default/base protection and other-worktree occupancy. Ordinary safe detach
preserves the folder, ignored files and checkout hooks. Those checks repeat after
detach; external changes cause conservative retention.

Deletion uses ordinary `git branch -D`, preserving its branch-in-use refusal.
A command-local prepared reference-transaction guard compares the actual direct
ref tip under Git's reference lock with the immutable saved OID, checks recovery,
current defaults and worktree-use snapshot, and vetoes mismatches. No update-ref
deletion fallback exists. Automatic deletion supports the qualified Git 2.50/files
subset only; other versions/backends, custom hooksPath or executable transaction
hooks retain the branch. Existing hook policy is checked before/after detach and
in prepared; the product does not silently replace a user transaction hook.
Reference writes request committed/reference fsync. The guard uses actual Unix
path bytes in argv/environment, never lossy path display.

This is **not a universal external-Git atomicity guarantee**. The process-local
lease excludes cooperating product workers only. Git's worktree-use checks are
immediate snapshots: ordinary concurrent `worktree add --no-checkout` can race
its own branch deletion check. Direct edits of Git internals, external guard/ref
changes and unobservable identical-tip delete/recreate (ABA) are not branch
identity tokens. Observable advances, rename/switch, symbolic replacement and
other-checkout use are retained. No remote branches are deleted.

Return is bound to the exact workspace/archive request (protocol 2), with durable
RestorePending before Git. Recovery and clean retained checkout must still match.
Absent old names are recreated; occupied names receive a fresh alternative.
Create-only branch and operation-marker refs are committed in one Git transaction;
ordinary safe switch preserves hooks and refuses another checkout. The marker
allows retry after create/switch success without assuming ownership of a reused
same-tip name. Durable Restored and active visibility commit together in SQLite.
Missing folder, changed recovery/HEAD/files or failed switch stay archived with
an explanation. Return never dispatches a coding launch.

Late intent acknowledgements cannot rearm cancelled branch work. Cancellation
of a running Return keeps archived visibility and any partial Git result. Once
completed Git results are already queued for durable storage, cancellation says
it is too late rather than promising rollback. Lost branch-worker results retain
the Archived save receipt and mark branch uncertainty separately. Restart loads
pending operations without automatically mutating Git. Absence after a lost reply
is uncertain, not attributed deletion proof. Duplicate completed Return is a no-op,
even if active work has since become dirty; stale Return cannot target a later
archive. Workspace/native/creation identities are unchanged and future PR/review
identity remains independent of the restored name.

## Demonstrated checks

The implementation used public Git worker, Controller/supervisor, and App keyboard
seams. Initial cleanup and restoration slices observed red before green. Review
regressions actually reproduced reference-hook installation after detach and a
same-tip symbolic branch replacement before their fixes.

- **40 archive Git tests** comprise 15 existing checkpoint cases, 16 cleanup cases
  and 9 restoration cases. Cleanup covers protected main/master/trunk/local and
  remote bases/default names, external tip advances, provenance, dirty staged/
  unstaged/new files, absent/changed recovery, another checkout, custom hook
  policies, post-checkout edits of tip/default/symbolic ref/hooks, packed refs and
  lost-reply absence/replacement. Restore covers GC, same-tip occupied old names
  and occupied alternatives, ordinary switch refusal, rejecting/mutating hooks,
  lost switch reply, cancellation after atomic branch/marker commit, missing
  folder/recovery and external work.
- **140 binary tests passed**, 3 intentionally ignored: two explicit retained
  native-corpus qualifications and the subprocess probe driven by its PTY test.
  This includes all App/UI/persistence regression tests and a new keyboard policy
  test for provenance defaults, exact-target adoption and submitted immutability.
- **11 Controller/dashboard archive scenarios passed in 33.30 seconds**, including
  all 7 ticket 16 scenarios and four new branch scenarios. Product-created default
  cleanup retains ignored files; GC and reused names survive restart; an injected
  restore-result storage error keeps archived visibility after Git switched,
  and explicit retry after restart uses the same branch. Duplicate Return preserves
  later dirty files; stale Return refuses the next archive revision. A supervisor
  crash while cleanup-result persistence is stalled restores pending archived
  metadata; explicit absence reconciliation and later branch-name reuse never
  delete the replacement. Cancellation while archive/restore intent storage is
  stalled does not rearm on acknowledgement. A real Git wrapper advances the tip
  **after the last app check and before porcelain deletion**; the prepared guard
  rejects with `ref updates aborted by hook`, retaining the new commit and recovery.
- An earlier replay of the ticket 16 history-deletion scenario exposed a completed
  archive receipt unnecessarily reissuing Stop, invalidating final-capture facts.
  Replay now stops execution only for an admitted archive operation; the scenario
  passes again. Keyboard qualification now waits for actual asynchronous Return
  completion instead of matching an already-visible Focus label.

Final query hardening preserves raw config success independently of UTF-8 output
and uses NUL-delimited worker occupancy records. The 16 cleanup tests and the
Controller expected-tip guard scenario were rerun after that change. Locked build,
formatting, warnings-denied all-target Clippy and `git diff --check` pass.

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --bin agent-terminal
cargo test --locked --bin agent-terminal archive_branch:: -- --nocapture
RAT_ARCHIVE_EVIDENCE=docs/proofs/screens/m2-17 cargo test --locked --test ownership archive_ -- --nocapture
cargo test --locked --test ownership archive_branch_guard_retains_tip_advanced_after_last_check_before_delete -- --exact --nocapture
```

## Layout and limits

Actual parsed outer-PTY captures at 160×40 and 60×18:
[normal preview](screens/m2-17/archive-160-preview.txt),
[normal receipt](screens/m2-17/archive-160-receipt.txt),
[normal return](screens/m2-17/archive-160-returned.txt),
[small preview](screens/m2-17/archive-60-preview.txt),
[small receipt](screens/m2-17/archive-60-receipt.txt),
[small return](screens/m2-17/archive-60-returned.txt).
They exercise adopted default-off, explicit `b` selection, protected-main cleanup
retention, checkpoint receipt and asynchronous stopped Return. The normal view
retains reviews over the project/worktree sidebar beside the prominent session
panel, compared with the vision image. The compact layout hides that sidebar as
previously documented; the exact target, toggle, archive/cleanup result and local
save coverage remain readable. Scrolling accesses the remaining receipt. These
are terminal-text captures, not graphical Terminal.app screenshots.

The APFS fixture filesystem rejects invalid UTF-8 filenames with EILSEQ. The
expected-tip guard scenario therefore exercised a temporary path containing
spaces/Unicode; actual non-UTF8 temporary-path execution remains unverified on
this machine (the argv implementation preserves bytes). On other Git versions,
deletion-specific Controller scenarios explicitly skip; public Git cases assert
conservative unsupported-version retention instead of claiming qualification.

No full all-target runtime suite, real Claude/Codex branch/archive qualification,
physical reboot, power-loss durability or universal concurrent checkout protection
is claimed. Existing ticket 16 capture/Stop and prior native evidence retain their
recorded scope. Abrupt loss during Git may leave locks; uncertain locks are never
automatically removed. Transcript deletion does not remove archive or restoration
refs. Those refs are retained independently and are not automatically expired.
