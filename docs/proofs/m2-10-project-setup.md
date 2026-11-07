# Configured project setup: scoped evidence

Ticket 10 adds optional configured project setup between retained workspace
creation and an explicitly requested native launch. This is a coding-workspace
execution boundary, with separate creation, setup and native receipts. It does
not establish Milestone 2, release C6, or new native-tool compatibility.

## Source and environment

Worktree `/tmp/rat-m2-10`, branch `feat/m2-10-setup`, started from exact integration
`fb60d1ab9fe490a201d05e7a8cec2ec88864d57e`. Ticket 11 code `8844b88` was merged,
then latest integration `e715dc1be8e6dd8137b248661544bff718758341`, including its
proof and completion. Schema 7 runs before setup schema 8. The concrete execution
helper's three scoped commits are preserved as `2bf8aae`, `4637429`, `49e6317`.
Private-policy recovery is `ca86278`; final product source is
`343c56184a243187bf64d850d292228ba3710021`. Its final locked debug build SHA-256 is
`2101159d6ac5e48407d5055dceadf4d24ec796dc188df8bf0414700bcc7d9927`.
The four outer-PTY text captures were refreshed by the final 11-case aggregate
on `7dd903d`, before the final read-only historical receipt correction.

Host: macOS 26.2 (25C56), arm64; stable Rust 1.99.1, Git 2.50.0. All execution
cases use temporary real Git repositories, actual `/bin/sh` scripts, isolated
explicit private state directories, actual supervisor/writer/Controller transport,
and a scripted native wrapper where coding launch is observed. No hidden provider
or additional real-agent turn is used. Physical reboot and current Claude/Codex
compatibility through this changed setup path remain unverified.

## User-visible result

`U` edits the selected project's optional `/bin/sh -c` command. Empty disables
setup. A new creation atomically freezes that private policy outside the client's
immutable creation intent. The normal creation handoff uses the original policy,
including after project settings change or an original operation is recovered.
No configured command means no setup process or setup receipt.

Setup runs in the exact newly created canonical checkout, also for Create without
starting. Its receipt occupies the existing session pane through `O`; it shows
phase, errors, bounded combined stdout/stderr tail, discarded count and explicit
Retry/Continue choices. The existing reviews-above-project-tree composition
remains. Actual outer-PTY captures at 160×40 and 60×18 show both armed and stopped
continuation labels in [screens](screens/m2-10-setup-60x18-armed.txt). The small
layout retains its existing hidden sidebar. These captures establish keyboard
behavior/layout in the fixture; they are not Terminal.app/iTerm visual or native
CLI qualification.

A failed setup gates bare native Launch/Resume, including an adopted symlink alias.
`T` explicitly retries the frozen command with a fresh attempt UUID; prior attempt
facts remain. `Y` commits an explicit skip. The receipt says whether Continue will
start the selected agent under still-live Create and start permission, or keep
coding stopped. Restore and reconciliation never reconstruct that permission
from persisted `intent.start`. Configuration changes cannot rewrite a prior
attempt or turn duplicate Create into a different immutable intent.

## Demonstrated scenarios

The `configured_setup_` Controller/dashboard cases exercise the actual worker
and sole-writer boundary:

- Success runs once in the exact cwd; Create without starting never launches
  coding; final bounded tail is retained independently of native history.
- Exit 7 retains workspace and error. Bare Launch through a canonical alias is
  refused. After project command changes, explicit Retry still executes the
  original command once more, then launches the original requested native wrapper
  once, with original creation intent retained.
- Continue after failure executes no second setup. It starts coding only when
  its displayed original Create and start permission remains armed. Targeted Stop
  on a second failed setup disarms its permission and leaves an unrelated running
  native session intact; subsequent Continue keeps coding stopped.
- A stdout/stderr flood exceeds the 8192-byte tail while navigation and targeted
  Stop remain usable. Cancellation reaps the actual setup leader, clears pending
  launch and prevents native execution. A separate scripted coding stream retains
  its own output and receives exact later input while setup floods and is stopped.
- Stalled prepare, authorization, Running and terminal saves each retain responsive
  targeted Stop. Before gate release the command writes no marker. During a
  Running save stall, cleanup is observed before storage is unstalled. Late
  positive acknowledgements never restart cancelled coding.
- A failed terminal save retains the exact result for explicit storage repair.
  Controller disconnect/reconnect observes the same attempt. Repair commits it
  without rerunning its external effect, and native launch proceeds once only
  if permission remains armed.
- Supervisor loss after command cleanup but before its final receipt restores
  uncertainty, no automatic command replay and no armed native launch. Same-boot
  leader absence remains conservative about escaped descendants. Ordinary quit
  refuses a false stopped acknowledgement. An injected proof boot identity change
  exercises reboot-evidence logic; reconciliation is read-only, then a separately
  explicit informed Retry executes once and keeps coding stopped. This is not a
  physical reboot claim.
- A delayed Stop frozen against the empty coding owner follows only that setup's
  original native handoff. After its actual process/history reservation settles,
  a new independent conversation starts; the same stale empty-owner Stop cannot
  signal that later conversation. An unrelated launch query cannot replace the
  original native result. Across restart, the original request retains its exact
  saved session UUID, Stopped lifecycle and outcome while the older stopped
  session remains absent from live runtime owners. Receipt reads leave selection
  and execution/capture reservations unchanged. This restart extension initially
  failed (original binding was unavailable); the correction collects bounded
  typed historical metadata during the existing SQLite restore before old final
  rows are excluded from execution ownership. It creates no extra runtime owner.
- Creation-to-first-setup receipt loss initially failed: the created checkout
  restored with no setup reservation. The correction freezes the optional policy
  with creation intent. A completed creation with a durable configured policy and
  no prior attempt now restores a durably saved known-never-started failure, with
  no process. The regression changes project configuration and resends the same
  original Create request, confirms unchanged client intent and original policy,
  rejects bare Start, then explicitly retries the original command once while
  coding remains stopped. Legacy creation records without a policy are not
  retroactively guessed. No archive, cancelled flag or original no-start fact is
  rewritten by this read-only-effects recovery.

The initial missing-configuration test failed before implementation. The crash-gap
test failed on the pre-policy implementation and passed after the correction.
An intermediate broader unit run exposed combined-footer truncation; the normal
and compact help were shortened and the full 17 UI checks passed afterwards.
An initial actual-PTY test also exposed uppercase `O` modifier handling; the
shortcut now accepts Shift, and normal/small receipt tests pass. The bounded outer
PTY harness drains output while awaiting quit, avoiding a test-only backpressure
hang; the product's native accepted-output ownership remains unchanged.

## Ownership, durability and bounds

Prepared commits before spawn. Spawn installs the std child immediately, before
fallible pipe/Scope/identity setup. A fresh process session/group, authenticated
gate and original boot/start evidence precede release. Only the exact attributed
Authorized commit releases the gate. Running and final facts commit independently;
fast successful commands need not keep a leader alive after the exec EOF receipt.
Success cannot launch coding until matching terminal commit and process/capture
cleanup. Stop remains available while any storage phase is stalled or failed.

The existing live Scope/lifecycle termination retains the child on unconfirmed
cleanup. Terminal `cleanup_confirmed` is separate from command-outcome certainty.
A restarted owner never signals saved PIDs. Latest canonical setup reservations
participate in native admission (including Resume), the 32-execution/capture cap,
and SessionSummary reservation observations. Future whole-workspace mutation must
cancel pending launch permission and include these receipts; a stopped empty
coding owner alone is insufficient. This change adds no archive/removal control.

One setup worker is admitted at a time. Commands have a 4096-byte limit; there are
at most 32 retained setup attempts, each at most 64 KiB serialized. Two reads of
4096 bytes per pipe per poll and one 8192-byte coalesced raw tail bound capture.
Output is labelled incomplete/truncated where applicable and controls are inert.
Live progress is transient until a phase receipt commits; it does not claim full
raw setup history or the native transcript durability guarantee. Setup commands
have no short inferred completion timeout. Gate connection/exec receipts have
five-second bounds; control polling is 10 ms and final pipe drain 250 ms.

Uncertain live cleanup retains ownership/reservation and retries the existing
bounded lifecycle verifier; a lost cleanup worker cannot acknowledge success.
A wholly lost setup worker becomes an uncertain receipt. These rare internal
worker-loss/output-error paths are conservative code-reviewed behavior, not newly
fault-injected proof. Scope sampling and reboot limitations remain those of the
existing native foundation. Arbitrary external/user shell activity is outside
managed ownership. The setup worker grants no unrestricted future-reviewer path.

## Final scoped validation

- `cargo build --locked`, `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`, and `git diff --check`
  pass for final product source `343c561`.
- `cargo test --locked --test ownership configured_setup_ -- --test-threads=1
  --nocapture`: 11 passed, 28.82 s on `7dd903d`, covering configured setup,
  private-policy recovery and actual normal/small receipt keyboard controls.
- The expanded original-receipt/later-conversation/restart Controller regression
  passes after the final historical metadata correction: 2.25 s.
- `creation_without_start_retains_exact_base_and_same_request_after_owner_restart`
  passes again on final source: 2.55 s. This preserves the no-config/no-setup path.
- `version_one_metadata_migrates_through_real_supervisor_without_losing_history`
  passes on final source: 1.85 s, retaining original metadata and history through
  schema 8 and the typed presentation read.
- The broader filtered bin run on `7dd903d` passed 95 tests with 2 explicit native
  corpus replays ignored and both terminal PTY tests excluded: 18.73 s. It preceded
  the final historical presentation correction; the changed seam was then tested
  through the expanded real Controller restart and legacy migration cases above.
- Earlier assembled-source regressions passed: original dashboard Create and
  start/native-failure retention (3.86 s), and ticket 11 actual keyboard retained
  search/passage opening at normal/small geometry (8.33 s). Final 17 UI checks
  passed after the combined setup/search footer correction. Mixed-case/punctuation
  input checks preserve shell text including all global shortcut letters.

Fixture/native windows were coordinated with tickets 11/12. Independent static
or unit work overlapped some development checks; these contextual durations are
not a benchmark or a new native CLI qualification.
The whole milestone all-target/native/resource aggregate remains ticket 19's
separate gate. This ticket did not run an unfiltered all-target aggregate or the
terminal restoration PTY test. All owned setup/scripted fixture processes were
reaped; the final process check found no remaining ticket 10 test/supervisor child.
No physical reboot, escaped-descendant fault injection, rare internal worker-loss
injection, or current actual Claude/Codex setup path is claimed here.
