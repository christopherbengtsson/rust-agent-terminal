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
Actual sleep/wake remains unverified until its real machine transition is
recorded. Neither case can be claimed from prior feature proofs. C6 and milestone
completion remain unclaimed while a required case is unverified.

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
Further focused checks and the corrected frozen full aggregate remain required.

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
fair capture and scoped final flushing. These focused results still require a
new clean all-target aggregate before the fixture/native qualification intervals.
