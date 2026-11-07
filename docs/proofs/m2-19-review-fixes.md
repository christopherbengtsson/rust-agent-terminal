# Milestone 2 review fixes and qualification delta

Ticket 19 remains **Resolved: no**. Actual sleep/wake, physical reboot and C6
remain open. The [qualification entry](m2-19-qualification.md) retains all earlier
failed and successful runs at their original source/binary boundaries.

## Source and review boundaries

The independent review axes examined `55f1e17..e3cf58e`. Standards reported zero
hard violations and one optional duplication finding. Spec reported the P2
completed-archive/removal diagnostic and P3 editor-position defects. Production
fix commit `2523e947b644f531633fca4f9cb8da2ea7abf596` addresses those findings:

- Spec P2: admission still refuses archived, removed, pending, loading, occupied
  or uncertain execution. Completed archives explain Return/unarchive; completed
  removals explain recovery into an available checkout and explicit adoption.
  Pending receipts and unavailable receipt loading retain separate refusals.
  No missing folder is offered as a place to resume or start coding. Native-ID
  absence remains a distinct unavailable-resume case.
- Spec P3: capture supplies bounded scalar source-text and selected artifact
  positions. Plain new-file source follows the visible line; rename and
  abbreviated-label comparisons open the selected raw path's section. Display
  labels are never identity keys. Editor capability 2 refuses older owners;
  source validation, comparison identity, limits and worker placement remain.
- Standards optional finding: Working, Whole and export share one local per-file
  presentation helper. Comparison headers and export caps remain at their
  existing callers. The Whole conflict explanation remains explicit.

Only `src/app.rs`, `src/editor.rs`, `src/session.rs` and `src/working_diff.rs`
change production behavior. There is no Cargo/dependency change. Source tree
`b1c2ad000e1e1bcc269f116b1a9f6a5a67f84f50` differs from the earlier qualified
`33cc9a17fffb6b72b4ede4dc537f377d6be58bd6`. The new product executable SHA-256 is
`92bbb1a0cd9bb8df6a85a4d226bb73859b467f9cf8fd10dda3fab977ba2e33e9`.

The later `41dba345fe67127f97f7214588da3b1cc0e401fe` changes only an explicitly
ignored measurement harness and its macOS process-identity helper. Its production
source tree and executable are identical to 2523e94. The evidence below separates
these sources. It does not transfer the old native-provider or performance
qualification to the new binary. Claude 2.1.318 supported / 2.1.319 Unknown and
Codex 0.167 scopes remain those of their original proofs; actual providers were
not launched during this fix qualification. No status monitor was widened.

## Regressions and focused checks

Real temporary Git repositories and public App/editor-worker boundaries first
failed with source line 1 instead of 4 and comparison line 1 (`Working`) instead
of the selected rename heading. Those initial command outputs were observed in
the tool session, not retained as standalone raw logs. The retained removed-folder
red regression uses an actual settled Archived + Removed receipt and known native
UUID, and reproduces the misleading pending-receipt message. Its green check
asserts unchanged managed/native identities, session list, no execution, and no
second scripted-provider invocation. Pending selected-resume cancellation and
loading refusal with an unreadable on-disk receipt are separately exercised.

A prefrozen serial main-target run caught a conflict-heading regression in the
shared helper: **158 passed, one failed, four ignored**. That failure remains in
the archive. Restoring the Whole conflict explanation made the complete affected
11-case Git diff group pass. This exploratory run is not a successful aggregate.

Clean source 2523e94 then passed the following serial focused groups:

| Boundary | Result |
| --- | --- |
| Dashboard editor PTY, including actual installed Vim/Pico | 2 passed, 10.70 s |
| Editor/App/source/artifact positions, including installed Vim | 6 passed |
| Original-ID resume, pending lookup, archive/removal refusal | 7 passed, 25.48 s |
| Unavailable archive-receipt loading refusal | 1 passed |
| Missing-checkout readable history and refusal | 1 passed |
| Canonical alias/pending removal and no deletion replay | 1 passed |
| Working/Whole/export with real Git repositories | 11 passed |

Actual Vim observes source line 4 and the selected rename section at artifact
lines 18 (Working) and 21 (Whole), for both ordinary and over-256-byte rename
labels. The terminal-editor check includes source saves, Ctrl-C, resize,
foreground group ownership and supervisor continuity with Vim 9.1 patches 1–1752
and Pico 5.09. These are editor qualifications on this source/binary, not native
coding-provider qualifications. The host is macOS 26.2 build 25C56 arm64, Rust
stable 1.99.1. Default Git is Homebrew 2.50.0; the isolated ownership wrappers
use `/usr/bin` Git 2.50.1 (Apple Git-155). Both versions are recorded.

Stable locked build, format check, all-target warnings-denied Clippy and
all-target prebuild passed on the final harness source 41dba34. Every recorded
executable and clean source state remained unchanged across each frozen focused
or measurement interval. A final integration-wide serial all-target run is still
required; none is claimed by these focused groups.

## Retained native-record refusal without coding

The immutable `~/.rat19-proof-a56eee5` state was copied to separate versioned
private directories, omitting socket/lock/endpoint files. Each copy used a fresh
owner handle and the new binary. Original SQLite/transcript hashes remained
unchanged. No provider process was launched.

The first cold probe failed a harness equality assertion because unrelated
restored stopped owners were still settling their startup reservations between
frames. Its failure is retained. Subsequent probes wait for those exposed
stopped-owner obligations to settle; the independent same-boot Interrupted
reservation remains present.

The final copy `~/.rat19-reviewfix-v3` demonstrates:

- Workspace `803ff645-4dbb-49fc-8fa4-275f108f530a`, segment
  `0cc0c78c-7a00-4102-b180-ff567c6f02d9`, native UUID
  `be6e219c-edbb-4bb2-8ca7-8f79f72879e7`: completed Archived/Removed,
  absent folder, accurate recovery-reference/adopt refusal, unchanged session
  list/native identity and no execution.
- Workspace `2ff443dd-42d0-4738-a0cd-46e326ede1ec`, segment
  `c89c6420-5a76-40e2-b04d-aa234dc76b89`: native UUID missing, distinct
  unavailable-native-resume refusal, no new segment/process.
- Cobalt's same-boot Interrupted reservation survives. No saved PID is signaled.
  The copied owners were cleaned up only through their freshly held subprocess
  handles; the original owner/state was never started or mutated.

The exact new binary is retained at
`~/.rat19-reviewfix-v3/bin/agent-terminal`. The final process audit found no live
matching owner or native fixture. These copies are refusal evidence, not prepared
physical-transition checkpoints. A new mixed-state preparation and current
handles remain necessary before either external physical gate.

## Failed measurement, cold-copy correction and fresh pass

The first new six-stream run on 2523e94 failed after its runtime windows and
shutdown, while querying a copied SQLite database read-only:
`SQLITE_READONLY_ROLLBACK` (extended 776). It had a hot rollback journal. The old
harness waited for endpoint unlink, which did not establish owner process exit;
transaction consistency of that failure-time copy is **unestablished**. This run
remains failed even though its recorded healthy attaches were visible.

A separate diagnostic copy reproduces the read-only error. Letting SQLite
recover that diagnostic copy returns `integrity_check=ok` and six valid partial
index frontiers. Its main-database hash is unchanged by rollback. The failed
original database/journal remain untouched. This diagnosis does not retroactively
establish the first copy's provenance or make that run pass.

The ignored-harness-only correction binds the authenticated peer to Darwin
microsecond process start and exact executable, rechecking start around path
inspection. After shutdown it waits for that same identity to disappear with
ESRCH; it sends no signal. It then verifies cold source/copy bytes and SHA-256,
preserves an untouched raw DB and all existing sidecars, and permits SQLite
recovery only in a separate analysis copy. Integrity, final accepted/written/
durable equality, exact retained raw extents, index bounds and accumulated
failure assertions remain. A surviving journal after verified exit would be
recorded explicitly, not discarded or described as a clean close.

The fresh frozen run on clean 41dba34 **passed in 76.00 seconds**. Its authenticated
peer identity was recorded before execution and absent before copying. This run
had **no journal after verified exit**; the raw/source/analysis DB hashes matched
and SQLite integrity passed. It therefore supplies the previously missing cold
provenance without reclassifying the failed run.

For the declared three-project/six-fixture-session workload, healthy visible
navigation worst was 8.022 ms and peer-fault navigation worst 4.709 ms. All 18
UUID-verified prepared-owner attaches were visible below one second, worst
897.364 ms; explicit owner selection and dashboard startup precede that timer.
The faulted stream's attach remained censored, not a healthy pass. Healthy capture
age stayed below 557.888 ms and receipt gaps below 410.236 ms. All six final
accepted/written/durable extents matched their retained raw byte lengths with
empty queues. Search covered the committed derivative prefix, and indexing
remained partial. RSS/disk samples and all per-stream counters are retained;
finite fixture results do not establish provider throughput, graphical latency,
allocator maxima, power-loss safety, actual sleep/wake or physical reboot.

## Artifacts

The [review-fix archive](m2-19-review-fix-evidence.tar.gz) and
[per-file manifest](m2-19-review-fix-manifest.json) retain original findings,
source patches, static/focused logs, the original failed measurement and its
DB/journal/transcripts, separate recovery diagnosis, corrected measurement with
untouched cold SQLite, before/after executable manifests, cold-copy probes
(including failures), process audit and tool versions. Earlier qualification
archives and pinned source/binaries are unchanged. Ticket 19 and the spec remain
**Resolved: no**; final aggregate checks and physical gates remain explicit.

## Integration merge checks

The clean `fix/m2-review` tip `b58ba9cc14fb3d58b2e31dc92cde88048b76bae7`
was integrated with a no-fast-forward merge at
`e65668c35c161d2f138558e4f6b122cc87db2163`, retaining the prior integration
`e3cf58ed2c0b5615204b69e0029dff7c3f47d981` as its first parent. On the merged
tree, Rust stable 1.99.1 passed `cargo +stable build --locked`,
`cargo +stable fmt --all -- --check`, and
`cargo +stable clippy --locked --all-targets -- -D warnings`.
All 700 archived regular files (86,748,553 uncompressed bytes) matched the
manifest's exact member set, byte lengths and SHA-256 hashes.

These checks do not rerun runtime, native-provider or physical-transition
qualification. The final frozen serial all-target gate follows this evidence
commit. Ticket 19 and the spec remain **Resolved: no**; actual sleep/wake,
physical reboot and C6 remain open. The clean fully merged temporary fix
worktree was removed while retaining its branch and the private qualification
source/state/binary pins.
