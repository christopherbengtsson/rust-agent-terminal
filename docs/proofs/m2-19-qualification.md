# Milestone 2 integrated qualification

Ticket 19 remains **Resolved: no**. This record will distinguish checks completed
on one frozen source/binary set from outstanding gates. Prior feature proofs are
supporting evidence with their original source/tool boundaries; they are not a
new integrated qualification.

## Frozen aggregate procedure

`scripts/m2-19-gate.py prepare <new-external-directory>` runs stable locked build,
format, warnings-denied all-target Clippy and prebuilds every target. After source
is committed and frozen, `run <same-directory>` records the source, clean state
and SHA-256 of every executable before and after the serial locked all-target
`--nocapture` command. Builds, source edits and unrelated native/PTY/resource
workloads must remain outside that interval. An aggregate failure remains a
failure even when a later focused check succeeds.

## Required evidence

- A declared three-project/six-session coding-only fixture workload, visible
  navigation/attach timing, per-stream capture/durability/queue bounds and measured
  process/storage growth during history search, diff and selective storage faults.
- An integrated keyboard workflow with six actual native coding sessions,
  creation/setup/search, original-ID older conversation recovery, both diff modes,
  real Vim/Pico return, checkpoint-backed archive/branch cleanup/Return and
  separate safe folder removal with retained history and local code recovery.
- Current native tool versions, actual prompts/questions/approvals, Ctrl-C,
  resize/focus escape, same-session attach, scoped Stop, quit and reconnect.
- Actual detach, ordinary quit, dashboard loss, supervisor crash, sleep/wake and
  physical reboot recovery with no duplicate or automatically restarted coding.
- Normal 160×40 and compact 60×18 captures compared with the vision image.

## Open external gates

No physical reboot is authorized or performed by this record. The spec requires
explicit user permission before reboot. A durable handoff must identify retained
state, source/binaries, session/native IDs and the exact post-boot checks before
that action is requested. Injected boot fixtures do not satisfy this gate.
The later actual sleep/wake follow-up below is now verified with OS power
evidence and unchanged prepared identities/state. Physical reboot remains
unverified; neither prior feature proofs nor injected boots satisfy it. C6 and
milestone completion remain unclaimed while that required case is open.

This ticket does not qualify GitHub review automation, publishing, task PR
continuation, notifications or the combined release workload.

## First frozen aggregate and correction evidence

The first complete attempt used source
`e5b1fc50acf72ad6914e60f7916d7df952035a5c` and product SHA-256
`ca88d3f22b2f7040da7601c2847f45924520b0aec7920f741f9353762c5eb493`.
Stable locked build, formatting, all-target warnings-denied Clippy and all-target
prebuild passed. Source stayed clean and every recorded executable hash matched
afterward. The serial all-target command exited **101**: main 154 passed/3
ignored, editor 1 passed/1 ignored, ownership 106 passed/7 failed/3 ignored in
415.25 seconds. Scripted/screen targets were not reached. This remains a failed
aggregate; focused corrections below do not turn it into a pass.

The seven failures were archive branch crash recovery, archive with restored
uncertain setup, setup restart reconciliation, keyboard retained search, oversized
OSC capture, missing-checkout history recovery, and sustained four-phase capture.
The [preflight archive](m2-19-preflight-evidence.tar.gz) and
[artifact manifest](m2-19-preflight-manifest.json) retain the complete command logs,
source/binary boundaries, original isolated results and diagnostic failures.

The three startup cases passed their first isolated reruns, so no deterministic
isolated reproduction is claimed from that run. Logging-only repeats then caught
setup recovery and archive refusal naturally on iteration zero: the immediately
preceding public observation had `removal.ready=false` and archive receipt loading
in progress. Empty removal records after loading ruled out an actual removal
obligation. The branch-crash case passed ten logged isolated runs, including two
observations of the independent not-ready window, but failed in the unchanged
aggregate with that admission refusal. Only the affected test actions now wait
for their exposed prerequisite facts; all archive, branch, uncertain-execution,
explicit-retry and no-auto-launch assertions remain. Shared restart helpers and
startup admission gates were not relaxed.

Search first failed because the exact-word guidance had been replaced by selected
native-resume guidance. A rendered regression went red before restoring the search
rules at 160×40 and 60×18, editing and result states; the original resume guidance
remains. The keyboard test also waits for its actual matching result before
opening it, keeping the committed-passage and matching-content assertions.

The original missing-checkout test remained red in isolation. A delayed public
wire probe proved that a later `v` action sends a fresh request; it did not cover
the test's immediate restored-Exited frame. An App regression for that exact early
frame then went red: the original request was reused while its outcome was absent.
Explicit actions now allocate a fresh request for a terminal historical segment,
while retrying a still-unobserved new intent and pending/uncertain execution keep
their request identity. The child-free regression covers 24 terminal-state/action
combinations and 16 pending-state combinations. A storage-worker preflight also
reports a known missing current workspace before native-descriptor rejection,
retaining history and raw Unix path bytes; unknown legacy locations do not prevent
identity-only lookup. No native conversation UUID is changed by that preflight.

Both resource failures reproduced on the original executable. Separate public
Controller probes kept the exact baseline binary and changed only the existing
debug derivative-index commit fault. Normal OSC capture still lacked its marker
after 35 seconds (4,963,332 output bytes beyond its launch baseline); with that
index-commit fault, all expected 8,388,638 bytes completed in 5.025 seconds and
became durable in 5.339 seconds. Styled 4 MiB flood completion/durability changed
from 29.815/30.336 seconds to 2.621/2.902 seconds. Counters kept advancing in the
normal runs: this was throughput/backpressure, not a deadlocked parser or a false
equality check. The fault probe was diagnostic, not a production workaround.
Its first canonical-path harness error and first secondary-resume harness timeout
are retained separately and are not called passes.

The worker correction drains a bounded capture turn before the unchanged
512-byte index step: at most 64 blocks/64 KiB, ending after a completed write
reaches 2 ms; errors end that stream's turn. Rotation, payload/queue bounds,
control priority, committed-prefix acknowledgement, index transaction atomicity
and FULL/fullfsync settings remain. A blocking write can exceed the time budget.
The unchanged OSC test then passed in 6.36 seconds with its exact raw bytes and
memory checks (RSS 10,656→11,056 KiB). The unchanged sustained test passed in
347.32 seconds, preserving all four stream modes, queue/saturation assertions and
all 131,072 numbered records exactly once. Its temporary resource file was not
retained; the pass log and exact source diff/executable hashes are retained.
At that checkpoint, further focused checks and a corrected frozen full aggregate
remained required; their later results follow below.

The subsequent [focused correction archive](m2-19-correction-evidence.tar.gz)
([manifest](m2-19-correction-manifest.json)) records all nine exact checks passing
with product/test binaries and Rust/test/Cargo source hashes unchanged throughout:
the three startup cases, strengthened keyboard search, original missing-checkout
history recovery, concurrent workspace controls/capture/faults, capture flushing
while searches run, saturated storage, and delayed writes/sync. Their measured
run times were 4.93, 1.39, 1.59, 7.77, 2.41, 2.41, 5.32, 3.99 and 4.78 seconds.
The child-free persistence group also passed 14 with its two explicitly ignored
native-corpus qualifications, including identity lookup with an unknown legacy
checkout location, selected-resume retention/deletion, index restart/passages,
fair capture and scoped final flushing. These focused results did not replace a
clean all-target aggregate; the subsequent frozen pass below preceded both the
fixture and native intervals.

## Clean aggregate and final product boundary

The corrected, clean frozen aggregate used source `335ba44e980f2e448f86bafe85aa4bfac82b1e5d` and product SHA-256 `89382b1a23124461fbec2bd0944e4b012cfb8d7f056990119a6a02a777569154`. Stable locked build, formatting, warnings-denied all-target Clippy and all-target prebuild passed before the runtime interval. The serial command `cargo +stable test --locked --all-targets -- --test-threads=1 --nocapture` exited **0** in 792.733 seconds: main 157 passed/3 ignored, editor 1 passed/1 ignored, ownership 113 passed/3 ignored, scripted interaction 1 passed and screen 2 passed. This is **274 passed, seven explicit ignored qualifications**. Source stayed clean and every product/test executable hash matched before and after. No competing native/PTY/resource workload, build or source edit occurred during the interval. The earlier failed aggregate remains retained above.

Source `a56eee5b25196f236969b5f2051e3d85707f3e0d` then refined only the ignored six-owner measurement and its analyzer. Its production `src` tree is byte-identical (`33cc9a17fffb6b72b4ede4dc537f377d6be58bd6`), and the product executable hash is unchanged. Build, formatting, warnings-denied all-target Clippy and all-target prebuild passed again; the strengthened ignored measurement and actual installed-editor qualification passed against that pin. A second immediate default aggregate was deferred for this ignored-harness-only change. Final integration/review checks remain required at their own final source; no later product change inherits these native/performance claims silently.

The [clean gate/editor archive](m2-19-final-static-evidence.tar.gz) and [per-artifact manifest](m2-19-final-static-manifest.json) retain exact logs, all executable boundaries, versions, unchanged sustained-capture telemetry, search/removal terminal captures, a56 preparation and installed-editor results. The real macOS Vim/Pico qualification passed in 8.00 seconds: both comparison modes, live-source saves, Ctrl-C, resize, foreground process-group ownership, terminal settings and supervisor continuity. Actual editor versions were Vim 9.1 patches 1–1752 and Pico 5.09. The integrated native editor run below separately verifies real source path/argv before sending save keys.

## Six-session workload and native interactions

The [measurement and 319 supplement](m2-19-measurements.md) records both the original current-owner-only fixture result and the strengthened UUID-verified all-six-owner result, including every per-stream counter and the partial index boundary. In the strengthened declared workload, visible navigation worst was 10.049 ms healthy and 4.641 ms with a peer storage fault; all 18 prepared healthy owner attaches produced their fresh visible ACK below one second (worst 950.150 ms). Selection and dashboard preparation precede that attach timer. The faulted owner did not produce its ACK within two seconds and is not a healthy attach pass. Healthy capture completion age stayed below 531.540 ms and receipt gaps below 418.969 ms; all six final accepted/written/durable extents matched with empty queues. Indexing remained partial. These finite fixture observations are separate from actual native behavior.

The [main integrated native proof](m2-19-native.md) records the real three-project/six-concurrent-session workflow at source a56 and the identical qualified product. Claude 2.1.318 is selected through a proof-only PATH; the host-default 2.1.319 supplement retains its Unknown semantic status. Native versions, exact managed/native identities, actual interaction outcomes, original-ID recall, archive/removal recovery and all failed harness attempts remain explicit.

## Visual review and working instructions

The actual vision image was viewed alongside native-run terminal captures at 160×40 and 60×18. The [capture images](m2-19-visual/) retain the parsed visible cells and rendered PNGs. At 160×40 the dark/cyan composition keeps Reviews above browsed worktrees beside the prominent native pane. At 60×18 the compact layout hides the left sidebar and gives the native viewport 58×11 cells; current managed identity remains visible. These are parsed PTY screens rendered for inspection, not physical-screen or font latency evidence.

Long labels still clip: the one-row Repository header can show its label while wrapping a long path below the row; the compact Adopted status can also lose a long path. Normal Selection details and native cwd/context provide additional identity, and the managed UUID prefix remains visible, but these captures do not establish perfect path visibility at either geometry. This is a documented display limitation, not silently called a successful fit.

Browsing a sidebar candidate and selecting the adopted native owner are separate actions. The native run exposed this distinction when an initial comparison targeted the old sidebar candidate. That attempt is retained and unclaimed. For a retained workspace in another project, open `w`, highlight that workspace and press `f` while the picker is still open. Enter on a retained running owner can attach immediately; Ctrl-] restores dashboard focus. README now describes these actual controls and permits switching among concurrent owners without stopping their peers.

## Crash reservation and physical-transition handoff

The main active-owner crash restored the latest Cobalt Claude segment as
Interrupted with a conservative same-boot reservation, with no native process
remaining and no automatic coding restart. That reservation was preserved. An
authenticated ordinary Shutdown did not complete while it remained reserved;
the timed-out attempt is retained rather than called a normal-quit pass. The
zero-coding owner was then attributed through a fresh authenticated Unix
LOCAL_PEERPID and exact canonical executable/argv/cwd/start identity before
termination. Saved or text-matched PIDs were never authority. Earlier ordinary
quit with six native owners is separate successful evidence.

The [physical-transition plan](m2-19-physical-transition-plan.md) is explicitly
unexecuted. It specifies known original native IDs and a proposed mixed
running/stopped/archived/removed state across three projects, to be prepared only
after review/fixes/final integration checks. Its fresh checkpoint must precede
any user restart request. The cold closeout checkpoint and its same-boot negative
verification are preparation evidence; unchanged real boot UUID/boottime must
fail, and do not satisfy C6. The final handoff preserves the private state and
qualified executable outside `/tmp`; no coding is automatically started by
restoring an owner. Actual sleep/wake and physical reboot remain open.

## Proof tooling closeout

The repository driver now retains the actually exercised external adaptations:
initial dashboard focus escape, versioned runner hash, current parsed-cell
assertions, per-step foreground/terminal observations and honest ENOTTY reporting.
The strict supplement additionally checks the actual Darwin executable and exact
NUL-separated argv file operand before a separate editing-key step. Stable dirty
source is rejected. The reboot verifier binds its own hash, exact previous
selected workspace/segment/native context (or marks unavailable prior evidence
unverified), and source-defined reference roles: archive index/prepared trees,
primary archive/removal/restore commits and creation-base commits. It never
launches coding, signals a saved PID or substitutes a fixture for a real boot.

The [proof-guard archive](m2-19-proofguard-evidence.tar.gz) retains original
counterexamples, 18 passing pure regressions, baseline/corrected hashes and the
earlier [16-case archive](m2-19-proofguard-initial-evidence.tar.gz). The first
strict ref-role parser passed its narrow pure cases but failed on the real cold
state because it omitted valid archive `-prepared` trees and creation commit
refs; that failed actual capture and original parser are preserved. The final
parser adds those exact source-defined roles with wrong-type/name regressions,
without relaxing unknown-role rejection. Final verifier SHA-256 is
`47cec00b410c74fc3745a4a8c645170a8c7c0e66dfe167382348eacf32a1db01`.
Final cold checkpoints use this verifier; earlier hash-bound checkpoints remain
earlier evidence. Formatting, diff whitespace and all proof-script AST parsing
checks also passed for the documentation/tooling closeout. The root integration
review and final all-target checks remain required.

## Integration candidate checks

The candidate `217284bd4a17d6c659705030d055e453b6e2aad9` was merged without
conflicts into the integration branch as
`23847054765d3872f77ea39f62b4a15221a93320`. Its production `src`, `Cargo.toml`
and `Cargo.lock` have no diff from qualified source `335ba44`. On that merge,
stable locked build, formatting, warnings-denied all-target Clippy and whitespace
checks passed. Focused serial binary tests passed: App 37, UI 19, persistence
14 with two native-corpus qualifications explicitly ignored. The proof-tool
regressions passed all 18 cases; all six ticket 19 Python scripts parsed.
Logs are retained at `/tmp/rat19-integration-{build,clippy,app,ui,persistence}.log`.
These scoped checks introduce no new native, physical-transition or full
aggregate claim. Two-axis review and a final serial all-target run remain due.

The clean, fully merged implementer worktree `/tmp/rat-ticket19` was removed;
its branch was retained. The detached persistent source and private proof state
remain available for coordinated physical-transition preparation. An unrelated
untracked `strict-editor-supplement.log` was preserved for its owner's closeout;
it was not included in this integration commit. Ticket 19 and the spec remain
**Resolved: no**; actual sleep/wake, physical reboot and C6 remain open.

## Review-fix qualification delta

The [review-fix record](m2-19-review-fixes.md) separates the original Standards
axis (zero hard findings, one optional duplication) from the two Spec defects,
and records their fixes at production source
`2523e947b644f531633fca4f9cb8da2ea7abf596`. This changes the product source tree
and binary: it does not inherit a56's native/provider or performance claims.
Real Git/App/editor positioning, actual installed Vim/Pico, canonical/pending/
loading/missing-checkout and original-ID refusal checks pass. A separate copied
retained state demonstrates completed-removed workspace 803's refusal and the
ID-null unavailable case without native execution; the original cold state and
its same-boot Interrupted reservation remain unchanged.

The new six-stream run first failed on a read-only copied SQLite/hot-journal
boundary; that failure and its unestablished cold provenance are retained.
Ignored-harness-only source `41dba345fe67127f97f7214588da3b1cc0e401fe` then adds
fresh authenticated process-start/executable exit evidence, byte/hash-verified
cold copies, untouched DB/sidecars and separate analysis-copy SQLite recovery.
Its production tree/binary are identical to 2523e94. The corrected frozen
measurement passes in 76.00 seconds, with all six raw extents and 18 healthy
owner attaches verified. Final stable build, formatting, all-target Clippy and
prebuild pass; the root's final frozen serial all-target suite remains required.
Actual sleep/wake, physical reboot and C6 remain open; ticket 19/spec stay
**Resolved: no**. The linked record and archive preserve the precise source,
binary, workload and failure boundaries.

## Final reviewed integration gate

The two independent reviews at `e3cf58e` found no confirmed documented-standard
violation, one optional duplicated-presentation finding, and two spec findings:
completed archive/removal refusal guidance and editor positioning for new files
and renamed/abbreviated sections. The single fresh fixer addressed all three;
independent read-only validators confirmed the committed production delta at
`2523e947`. The [review-fix record](m2-19-review-fixes.md) retains red/green
regressions, actual current-source Vim/Pico checks, the copied-state refusal,
corrected six-owner measurements and the failed hot-journal-copy attempt.

After merge and scoped checks, final integration source
`59fed09a8c13505b68d93dd10e5612419a3d08d5` was clean and frozen. Stable locked build,
formatting, warnings-denied all-target Clippy and all-target prebuild passed.
The serial locked all-target runtime command exited **0** in **813.55 seconds**:
main 159 passed/4 ignored, editor 1 passed/1 ignored, ownership 115 passed/3 ignored,
scripted session 1 passed and screen 2 passed: **278 passed, eight intentional
ignores, zero failures**. Those explicit qualifications include installed editors
and six-owner measurements exercised separately at their recorded sources.
All source and every product/test executable hash remained unchanged throughout
the runtime interval, without overlapping builds, source edits or native/resource
workloads. Production src tree is `b1c2ad000e1e1bcc269f116b1a9f6a5a67f84f50`;
this worktree's product SHA-256 is
`b50530e0e859eb23fd7c61d4442d5fc8dd47c11aaffe39615897be76df460faf`.

The [final integration archive](m2-19-final-integration-evidence.tar.gz) and
[per-file manifest](m2-19-final-integration-manifest.json) retain preparation,
complete runtime logs, versions, before/after boundaries, sustained-capture
telemetry, and actual search/removal terminal captures. This later clean pass
does not overwrite earlier failed aggregates or silently move the exact a56
native observations to the new binary; the review-fix delta has its own evidence.

This fulfills the final code-review/runtime checkpoint. **Ticket 19, C6 and
milestone acceptance remain open** pending actual sleep/wake and explicitly
user-authorized physical reboot. The new mixed-state preparation is a separate
phase after this gate, with fresh source/executable/state pins; neither the cold
closeout nor a same-boot negative verifier is a physical-transition pass.

## Actual sleep/wake follow-up

The user-performed [actual sleep/wake check](m2-19-sleep-wake.md) passed on
2026-11-07 with OS power events and both prepared conversation/process identities
unchanged. Code, refs, membership, selected context and committed history prefixes
verified; an immediate pre-reboot checkpoint was refreshed. This supersedes the
earlier sleep/wake-open status only. **Physical reboot remains unverified, so
C6, ticket 19 and milestone acceptance remain open.**
