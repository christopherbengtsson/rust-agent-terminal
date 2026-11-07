# Ticket 18 — explicit recoverable workspace folder removal

2026-11-07, macOS 26.2 (25C56), stable rustc/cargo 1.99.1. Public Git
worker tests used Git 2.50.0; Controller/PTY supervisors used `/usr/bin/git`
2.50.1 (Apple Git-155). Implementation/source tests are commit
`f8fd7cc0de678a36a817e38e1680f362846f8f86`, descended from integration
`bbd7a7cf81b255b781b693458e6fd0920d4a1d9d` and completed tickets 16/17.
This is scoped folder-removal evidence. Ticket 19 retains combined native,
resource, sleep/wake and actual reboot qualification.

## Behavior and destructive boundary

`M` opens a separate removal preview, bound to workspace/request UUIDs. It shows
the canonical folder, exact HEAD, branch/detached state, recovery ref, local-only
warning and actionable blockers. `n` requests a fresh preview; `j`/`k` scroll.
Plain `y` confirms only that exact clean preview. Local-only code additionally
requires Space acknowledgement; Ctrl-Y cannot confirm. Enter never removes a
folder. Escape dismisses, `r` requests supervisor-owned storage retry, `g` performs
read-only outcome reconciliation, `d` detaches and `q` cancels pending mutation
while quitting. Git work already completed cannot be rolled back by cancellation.
Removal does not implicitly stop another conversation/setup.

The new concrete Coordinator/worker and schema 11 receipts remain separate from
archive visibility and transcript deletion. Saved commits, previous archive refs,
branches, session/native/workspace IDs and future PR identity are preserved.
Protocol capability 1 excludes incompatible owners before dispatch. Recovery
readiness is visible; setup/launch admission stays closed while receipts load.

The sole writer durably saves Preview, then confirmed Intent, before protection.
The Git worker creates or verifies
`refs/rat/removal/<workspace>/<request>` at the **exact current commit**, including
never-archived detached work. Reference writes request
`core.fsync=committed,reference` and `core.fsyncMethod=fsync`. The immutable
snapshot/ref/HEAD and Removing intent then commit through SQLite before the
separate worker can call ordinary `git worktree remove -- <canonical folder>`.
A hash alone is never the recovery guarantee. No force flag, branch deletion,
recursive saved-path deletion or startup deletion replay exists.

Protection/removal hold the per-common-directory mutation lease only during
bounded Git work. They release it before SQLite waits. Confirmation and subsequent
admission account for all relevant canonical owners and workspace aliases,
including active/uncertain execution, native-resume lookup, pending capture,
metadata/outcome obligations, setup/creation handoffs and incomplete archive/branch
mutations. Aliases and asynchronous resume completions use the same admission gate.

Main, missing, locked/prunable and replaced/unsafe identities are excluded.
Device/inode anchors bind the root, Git/common directories, `.git` link and
registration file. Tracked/untracked status **and an independent ignored-file
inventory** run at preview and again after confirmation. Immediate checks repeat
identity/HEAD/branch, registration, blockers and the exact recovery ref after
network/ref work, with a final independent ignored inventory before removal.
This matters because actual ordinary Git removal can delete ignored contents;
Git's own refusal does not replace that check. Observed races retain the folder
and its files; ordinary Git refusal remains authoritative.

Known absent upstream/no remotes is local-only saving requiring acknowledgement.
A configured missing/unreachable remote, missing upstream or uncertain live push
check blocks removal; acknowledgement cannot override those blockers. Live push
advertisements are bounded/fingerprinted, with interactive prompts disabled.
Changed remote evidence requires a new preview. A local upstream (`remote=.`)
is not remote backup. Unsupported special/sparse/submodule indexes, external
object stores, shallow/partial clones and operation/lock state are conservative
blockers rather than promises of full saved coverage.

Only the exact durable Removed completion reports success. Storage failure stops
further mutation. Restart loads incomplete intent without performing Git work.
Explicit `g` confirms removal only when root and registration are absent and the
exact protected commit still resolves; matching anchored retained state releases
that intent for a new preview. Replacements/inaccessible identity stay uncertain.
No reconciliation step deletes a path, recreates a checkout, or starts coding.
History remains usable without the folder; the receipt explains unavailable
in-place native resume and identifies the ref for ordinary Git code recovery.

## Demonstrated checks

The agreed public seams were real Git worker APIs, supervisor/Controller receipts
and admission, App keyboard transitions, and actual outer PTYs. The initial clean
Git slice and App preview/Ctrl-Y cases observed red before green. Subsequent cases
were added incrementally. A real reference-transaction hook inserting ignored
content established the protection boundary rather than relying on a mock status.

- **12 real-Git worker tests pass.** Never-archived detached code survives removal,
  expired reflogs and `git gc --prune=now`, and a lost reply reconciles without
  touching a replacement path. Tests cover unsaved tracked/untracked/ignored files
  at preview and confirmation, local-only/no-upstream and live bare-remote backup,
  changed/missing push destinations, missing configured/local upstream, index
  flags/locks/Git operations, main/locked/missing/prunable and replaced paths,
  stale recovery refs, HEAD advances/branch switches, branch/prior-ref retention,
  committed-ref hook edits, cancellation after ref creation, raw whitespace paths
  and unsupported alternate/promisor object coverage.
- **Final binary suite: 154 passed, 3 existing intentional ignores, 12.52 seconds.**
  Ignores are the two explicit retained native-corpus qualifications and the PTY
  subprocess probe driven by its parent test. All 12 Git tests and the two new
  App tests are included. The final targeted App pair and UI suite additionally
  passed (2 and 18 tests).
- **Seven Controller/outer-PTY removal scenarios passed in 30.12 seconds.**
  They prove exact detached-code/branch/prior-archive-ref retention and readable
  history after removal/GC, discarded confirmation reply and restart, replay
  refusal and read-only reconciliation; a durable-before-Git writer stall with
  alias launch/Archive rejection and supervisor loss; failures at Intent,
  before-Git and final-result storage boundaries; active coding and pending native
  resume lookup exclusion; active setup exclusion without implicit cancellation;
  incomplete archive branch mutation exclusion after restart; and real keyboard
  blockers/confirmation/result at both viewport sizes.
- **Archive regression group: 11 passed, one readiness fixture failed, 51.85
  seconds.** The existing uncertain-setup recovery fixture sent its explicit
  recovery immediately after restart, before removal receipt loading completed.
  It now waits for the exposed readiness fact; the exact corrected scenario
  passed in 1.94 seconds. The other eleven group cases passed. The entire group
  was not repeated after that fixture correction.
- Locked build, formatting, warnings-denied all-target Clippy and
  `git diff --check` pass. Source checks do not establish native compatibility.

The binary checkpoint initially exposed footer clipping from the additional
shortcut; the footer now retains Stop/history/quit/detach/native actions at both
sizes. A Controller test also exposed a rejection path skipping its response;
the corrected archive refusal returns a visible outcome while removal owns the
checkout. Early PTY fixture runs were corrected to wait for controller release
and the matching new creation receipt, rather than accidentally using an earlier
successful setup. An initial partial-name `--exact` command matched zero tests;
only the subsequent full-name test is counted above.

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --bin agent-terminal
cargo test --locked --test ownership archive_ -- --nocapture
cargo test --locked --test ownership archive_refuses_restored_uncertain_setup_despite_exited_process_and_cancels_without_mutation -- --exact --nocapture
RAT_REMOVAL_EVIDENCE=/tmp/rat-m2-18-evidence cargo test --locked --test ownership folder_removal_ -- --nocapture
cargo test --locked --bin agent-terminal app::tests::folder_removal -- --nocapture
cargo test --locked --bin agent-terminal ui::tests -- --nocapture
```

## Layout and remaining limits

Actual parsed outer-PTY captures:
[normal preview](screens/m2-18/remove-160-preview.txt),
[normal blocker](screens/m2-18/remove-160-blocker.txt),
[normal confirmation](screens/m2-18/remove-160-confirm.txt),
[normal result](screens/m2-18/remove-160-result.txt),
[small preview](screens/m2-18/remove-60-preview.txt),
[small blocker](screens/m2-18/remove-60-blocker.txt),
[small confirmation](screens/m2-18/remove-60-confirm.txt),
[small result](screens/m2-18/remove-60-result.txt).
Compared with the vision image, 160×40 retains reviews over project/worktree
navigation beside the prominent session panel. The existing 60×18 compact layout
hides the sidebar. Scrolling exposes full identity, local-only acknowledgement,
blockers and the final receipt; the two-line help retains recovery/navigation
controls. These are terminal cell captures, not graphical Terminal.app screenshots.

The APFS fixture filesystem rejects invalid UTF-8 filenames with EILSEQ. Actual
Git execution therefore covers spaces/newlines/tabs; non-UTF8 model/path encoding
continues to preserve bytes, while native non-UTF8 execution is unverified here.
The process-local lease excludes cooperating workers, not arbitrary external
editors/Git: files/refs can change after the last observable check. Git refuses
observed unsafe state, but no universal interprocess atomicity is claimed. Avoid
external edits while removing a workspace. Filters/LFS retain ordinary Git
semantics for external resources. Abrupt loss during Git can leave locks; no
uncertain lock is automatically removed.

Ref fsync plus FULL/fullfsync SQLite ordering is not an atomic transaction across
the two stores or a demonstrated physical power-loss guarantee. Refs have no
automatic expiration. Actual disk-full hardware, physical reboot, real Claude/
Codex folder-removal/native-resume interactions, combined workloads and the full
all-target runtime suite remain ticket 19. Scripted native-shaped children and
injected boot identities above do not relabel older native proof as new evidence.
