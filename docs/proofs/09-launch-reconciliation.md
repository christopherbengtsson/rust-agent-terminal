# Ticket 09: durable launch intent and conservative reconciliation

2026-11-06; macOS 26.2 (25C56) arm64, stable rustc 1.99.1, debug build.
The external seam uses real dashboards in 160×40 PTYs, real supervisor/helper/scripted-child processes,
temporary Git repositories and private state directories. No native Claude/Codex
interaction compatibility is claimed by these fixture runs; their integrated
native retest remains ticket 18. The temporary helper execs the selected native
process and is not a persistent per-session daemon.

## Crash and acknowledgement boundary

The existing single SQLite worker commits metadata transactions using FULL
synchronization. `Prepare` durably reserves task/session/request identity before
spawn. `Authorize` durably records boot/PID/start evidence before a release byte
can reach the gate. The gate's private socket includes a per-launch nonce and
closes on successful exec; exec failure sends an error receipt. The gate wrapper
prefixes the existing native command's argv in place, preserving its executable,
every lossless argument, environment and cwd. A real executable
stub verifies Codex `--no-daemon`, TERM and checkout through the dashboard.
The supervisor checks live process-start identity on a successful receipt, then durably saves
that result before replying. A missing receipt or vanished identity is uncertain,
not a successful native launch inferred from helper creation.

| Window | Durable evidence on restart | Recovery and tested outcome |
| --- | --- | --- |
| Before intent commit | No process was authorized or created | No coding can start; a failed/pending metadata operation is visible. Not an injected process crash in this run. |
| After intent, before process creation | Prepared stable identities and boot; no authorization | Interrupted/uncertain same-request outcome is committed; no owned coding child; replacement blocked until explicit `u`, then an explicit new request creates one child. |
| After helper creation, before authorization | Same Prepared record; helper has no release | Socket loss makes helper fail closed without native coding; same explicit reconciliation and one-child replacement proof. |
| After authorization, before release/exec receipt | Boot, PID, microsecond start; release may or may not have happened | Uncertain; no automatic replay. This narrower instruction-level window is conservatively covered by the state rule, not a separate injected barrier. |
| After successful exec receipt, before result persistence | Authorized evidence; no known result | Injected crash restores an interrupted/uncertain result for the original request; no replacement until supported reconciliation. |
| After result persistence, before acknowledgement | Authorized evidence and original successful result | Injected crash recovers the exact success for query/retry without another child. Survivor uncertainty independently keeps replacement blocked. |
| Lost dashboard acknowledgement | Durable original result in live owner | Existing lost-ack scenario recovers the same request/session and exactly one child. |

The four injected barriers are `before-create`, `after-create`, `before-result`
and `after-result`. Debug builds alone accept a deliberately created private
`proof-launch-crash` marker; the barrier removes it and `_exit(86)` simulates
supervisor loss without running destructors. No barrier changes working files.
The crash matrix checks the original request, unchanged outcomes on same-ID
retry, restored Interrupted lifecycle, zero newly owned coding children after
restart, explicit replacement refusal, known pre-release reconciliation and
one child after explicit replacement. The uncommitted sentinel file is unchanged.

## Survivor evidence and user consequence

macOS uses `sysctlbyname("kern.bootsessionuuid")`, avoiding wall-clock boot-time
changes, plus `proc_pidinfo(PROC_PIDTBSDINFO)` start seconds/microseconds. PID
alone is never enough. A matching live identity reports a verified survivor.
A reused identity, missing evidence or failed process inventory stays blocked;
recovery never sends a signal to a restored process. `s`/`q` leave Interrupted
unconfirmed when execution is not owned by this supervisor. `d` detaches.

The survivor scenario asks the fixture to ignore SIGHUP and remain alive after
its PTY owner dies. It saves a readable response, kills the real supervisor,
reopens, verifies the original survivor and refuses a competing launch. A signal-0
check confirms reconciliation did not kill it. The harness then explicitly kills
its own known fixture identity. Same-boot leader absence remains blocked because
escaped descendants cannot be excluded. Process-group inspection explains known
survivors but never treats an empty group as proof against escaped descendants.

The reboot-style case does **not** reboot this Mac. Only after harness cleanup of
that known survivor, a debug-only `RAT_PROOF_BOOT_ID` environment override injects
a different OS boot identity into the next supervisor. The real host boot remains
unchanged. Explicit `u` now clears exclusivity, with no automatically spawned
coding child; history and the original request result remain readable. An explicit
new launch produces exactly one child and preserves the uncommitted sentinel.
This establishes the boot-evidence state rule, not physical reboot/power-loss
behavior. A real verified different boot is the conservative product recovery
route for same-boot native descendant uncertainty.

A separate missing-native-executable case reaches the gate, obtains an actual
exec failure, durably retains that failure, crashes/restarts the supervisor and
retries the same request. It returns the same failure with zero coding children.
The adopted checkout remains available for a subsequent explicit fixture launch.
Native resume is never inferred from readable history or a replacement launch.

## Bounds, storage and remaining limitations

The 1024 durable request-result cap stays fail-closed. Prepared/authorized state
uses bounded existing worker commands/completions; no unbounded record load or
new writer is introduced. All persistence, process inspection and PTY work is
outside the UI loop. `output_bytes` remains the owner-lifetime screen boundary;
`durable_bytes` remains the current session's committed raw prefix. No new loss
or flush-time promise is made. Failed metadata remains visibly unconfirmed.

Legacy active or already-Interrupted records without historical boot identity
receive a separate durable `observed_boot` anchor on their first recovery
observation. They stay blocked on that boot; a later verified different boot
proves no earlier survivor can remain, without inventing their historical launch
boot. A literal version-one fixture tests this through same-boot restart and
injected later boot, followed by one explicit replacement child. A durable
reconciliation marker distinguishes successfully reconciled Interrupted records
from legacy Interrupted records that never had execution evidence. Native tools
escaping their process group after authorization, permission/inventory failures,
and PID reuse may require reboot rather than a same-boot replacement. The proof
does not claim exactly-once execution, arbitrary descendant cleanup, restored PTY
attachment, native conversation resume, physical reboot, or abrupt power-loss
safety. Successful reconciliation keeps Interrupted history and requires a new
explicit conversation launch. The single-session prototype still blocks adoption
of a different checkout while the uncertain reservation exists.

Reproduction:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets -- --nocapture
```

TDD observations: the initial external recovery test failed because no durable
process evidence existed. The crash matrix then failed because its first crash
barrier was absent; implementation made all four windows pass. A normal/small
rendering test failed at 70 columns because reconciliation help was clipped;
its compact recovery footer now passes. Missing-native-executable restart and
actual-survivor checks run through the same real-process seam.

After merging integration revision `f511471`, the full suite passed: 19 unit
checks plus one ignored subprocess probe driven by the passing terminal-restoration
check; 23 ownership/history/native-choice/recovery scenarios; one scripted session;
and one styled-screen integration. Its four crash rows printed zero owned coding
children on restoration. The screen's maximum encoded snapshot was 358707 bytes.
Locked build and formatting passed. Warnings-denied Clippy initially found two
collapsible-if lints; those were corrected and all-target Clippy passed. The
final repeat additionally covers the legacy recovery anchor and bounded gate
handshake; its result is recorded at the end of this document.
That handshake change exposed inherited nonblocking accepted sockets on macOS;
the worker now explicitly restores blocking mode before timed nonce/exec reads,
so incomplete readiness is never mistaken for a native exec failure.

Final exact-source validation after merging integration `ac3ea5a` into ticket
revision `42b9541`: locked build, formatting and all-target warnings-denied Clippy
passed. All-target tests passed: 20 unit checks (plus one ignored subprocess
probe driven by terminal restoration), 25 ownership/history/native-choice/recovery
scenarios, one scripted-session scenario and one screen integration. The four
crash windows again restored Interrupted with zero automatically owned coding
children. The maximum encoded 200×80 snapshot was 358817 bytes. The final native
monitor integration preserves Claude's canonical executable, invocation session
ID and settings arguments before wrapping the native command. These are fixture
and integration checks; native-tool compatibility revalidation and physical
reboot remain unverified by this ticket.
