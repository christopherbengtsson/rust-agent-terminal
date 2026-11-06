# Ticket 14: recoverable history-storage faults

2026-11-06, macOS 26.2 arm64, stable Rust 1.99.1, debug build. The approved
seams are real dashboard/supervisor/scripted PTYs in temporary Git repositories,
plus the existing App/render boundaries. Unix sockets and PTYs ran outside the
execution sandbox. These are scripted integration observations, not additional
Claude/Codex compatibility or physical-terminal presentation claims.

## Faults and durability

Debug-only files in the explicitly isolated state directory inject faults on the
sole persistence thread: `proof-storage-{write,sync,directory-sync,prepare,
authorize,save}` accepts `stall`, `error` (ENOSPC), or a bounded millisecond delay.
`proof-storage-partial` contains a prefix length: it really writes that prefix
to the actual transcript file, then returns ENOSPC. There is no second writer,
user-disk filling, automatic deletion, or product `tcflush`/discard path.
Release a stall by repairing/removing its own marker; a queued Retry cannot
interrupt an OS syscall already in progress. Production builds ignore the fault
files. A synthetic ENOSPC tests error handling, not actual full-volume semantics.

The partial-write test lands at byte 65530, six bytes before a real 65536-byte
rotation. Injecting a seven-byte prefix writes six real bytes and fails without
advancing the logical/durable prefix. Retry seeks the original offset and
replays the retained block, then rotates normally. Exact raw byte comparison
includes every known response byte and proves no duplicate or gap. This retains
the existing position-based append design. A failed new-chunk directory sync is
retried even though its empty file now exists. Delay cases use actual 250ms
writes and 1500ms syncs; directory-sync ENOSPC, persistent append ENOSPC and final
SQLite commit failure all recover after explicit repair/retry.

The durability boundary remains transcript `File::sync_all`, then the SQLite
FULL transaction commit of its extent. Metadata/result success is acknowledged
only after commit. Final progress counters now publish before the Flushed event
and stopped ACK. A written/partial file tail is not a saved prefix. Error state
clears after the retained block is accepted by storage and a genuine successful
durability boundary, rather than after a screen poll or empty timer opportunity.
Persistent append errors pause 100ms before their next ordinary attempt.

## Control while persistence is unavailable

Launch Prepare, Authorize and result Save return provisional observations with
`metadata_pending=true` and no successful `launch_result`. This releases the
existing half-duplex transport to service Stop, Shutdown, Screen and RetryStorage.
The supervisor continues accepting/refusing connections and reading commands.
Request-tagged Prepared/Authorized completions and an explicit cancellation latch
prevent a late commit from creating/releasing a canceled launch. Durable
pre-release authorization, the same-request result cache, unknown-request
recovery and restored survivor exclusivity remain in force.

Tests stall each of Prepare/Authorize/result Save, confirm a prompt provisional
reply, stop the owned process independently of the writer, release storage, and
verify no late coding execution. Stop during a stalled Prepare clears only that
current owner's pending reservation; it cannot clear restored unknown execution.
The survivor/reboot-style and launch-crash matrices remain regression checks.

`y` latches a storage retry even when the storage command queue is full. It never
spawns a replacement writer. A stalled worker must return before processing that
retry. `s` dispatches process termination through its independent lifecycle worker.
Stop can report Stopped while capture remains blocked; this does not acknowledge
history or quit. New launch/adoption cannot replace the old capture while its
EOF/final flush is pending. A test explicitly refuses replacement at that point.
Quit waits for successful capture drain, final flush and metadata; persistent
failure stays visible, and repair plus retry finishes the exact retained bytes.
The real readonly-directory final-commit failure test also remains passing.

The saturation loop exposed a native exit race: TERM succeeded, but KILL could
return EPERM while a PTY leader was transitioning to a zombie. A direct owned-PID
observation showed matching UID/EUID 501 and later Z state. Lifecycle now preserves
the signal error while continuing its original bounded observation; success still
requires actual child reaping **and** verified absence of live group members.
An observable group/failed inventory remains StopFailed at the original deadline.
There is no permission workaround, process-group inference from zombie alone,
or relaxed survivor reconciliation.

## Raw preliminary fault observations

Primary recorded-source observation: [14-storage-verified.jsonl](14-storage-verified.jsonl),
with [completed-write ledger](14-storage-verified.emitted.jsonl). It records clean
source `ccf2cb510f3a4f2aa3c3d3b7c82f48e7193437eb`, debug profile, queue caps,
PID/RSS samples, full observations and controls. The fixture is configured for a
4MiB sequence of 32768 numbered 128-byte writes, but this fault run explicitly
stops it after 79 completed writes; it is not a completed 4MiB throughput run.
The opt-in ledger adds real per-record file I/O and is unsuitable as an
uninstrumented throughput baseline.

| Observation | Final recorded value |
| --- | --- |
| Dashboard navigation to a different parsed Selection branch | 7.466ms, one sample |
| Retry protocol response while writer stalled | 4.136ms, one sample |
| Stop to confirmed process cleanup, history still pending | 663.215ms |
| Completed fixture payload writes | 79 records, 10112 bytes |
| Accepted payload bytes, excluding 99-byte initial fixture prefix | 9088 bytes |
| Accepted total before repair | 9187 bytes |
| Written/durable before repair | 99 / 99 bytes |
| Pending capture high water | 9088 bytes; actual queue saturation observed |
| Accepted/written/durable after repair | 9187 / 9187 / 9187 bytes |
| Pending bytes after repair | 0 |
| Completed full-queue producer transfer wait | 2.960239s |
| Producer wait observed while still in progress | 529ms initially, increasing during sampling |
| Dashboard sampled RSS before abrupt dashboard cleanup | 6800KiB |
| Supervisor/fixture RSS across ten short saturated samples | 6544 / 3040KiB |

The sample cadence is roughly 100ms sleep plus `ps`/request cost. These are short
sampled RSS observations, not allocation peaks, a sustained memory plateau,
release budgets or latency distributions. Dashboard RSS was sampled once before
it was killed; the later ten samples have no dashboard process. Navigation was
observed in the real outer PTY's vt100-parsed Selection panel, not physical pixels.
The initial native pane was attached during saturation before returning to
navigation. Usable attach/input-nonce latency under a storage fault is not
established here; the synchronous fixture cannot read its nonce while blocked
writing output. Ticket 17 owns the concurrent fixture, distributions and actual
fault attach measurements without waiving the one-second target.

Capture has 64 slots of at most 4096 bytes, plus one worker and one waiting
producer block: at most 270336 owned pending bytes, plus the reader's 4096-byte
stack buffer. Actual saturation can occur at much smaller byte totals because
PTY reads produce small blocks. Control/completion queues remain 16 each; input
and dashboard transport queues remain 32 bounded requests. The outer harness
now has eight 4096-byte blocks. Parser/frame bounds remain ticket 13's limits.
SQLite/allocator/OS overhead is additional; transcript disk history grows by
retention policy. The raw ten-sample queue occupancy stays fixed while storage
and producer wait ages increase. A full-storage error remains visible after a
failed quit, then repair/retry drains and compares all accepted bytes exactly.

Earlier immutable `14-storage-fault`, `14-storage-confirmation` and
`14-storage-final` JSONL/ledger pairs retain their original revisions and samples.
Their `fault` description incorrectly conflates the separate partial-write test
with this saturation scenario: saturation stalls the real write, then injects
ENOSPC **before** a new write. The verified record labels that exact boundary.
The prefix-writing ENOSPC evidence is the separate chunk-rotation regression,
not a claim that these raw saturation records used a partial-write fault.

## Loss boundaries and limitations

In the recorded Stop run, 10112 payload bytes completed fixture writes, while
9088 reached application capture. The remaining **1024 emitted but uncaptured
bytes** were not recovered after TERM/native PTY exit. The product did not issue
a flush/discard operation; termination/kernel PTY behavior is outside the
accepted-byte retention promise. All 9187 accepted bytes, including the initial
prefix, became durable exactly once after repair. The in-progress producer write
never completed and has no completed-write ledger row. Completed-write microseconds
(7µs maximum here) therefore do not describe that outstanding blocked write;
the ongoing capture wait and later completed transfer wait expose backpressure.
The dashboard retains a possible-loss notice after stopping during capture
trouble. Pausing PTY reads does not safely suspend arbitrary coding/descendants.

A separate abrupt-supervisor test writes 75 real bytes beyond a 99-byte durable
prefix while sync is stalled, then kills the isolated supervisor. Restart returns
the same identity as Interrupted, starts no coding child and reads only the
99-byte acknowledged prefix. The 75-byte raw unacknowledged tail survives in that
run but is excluded from readable committed history; it is neither counted nor
silently overwritten. Tail recovery after supervisor restart remains explicitly
fail-closed. The older durable-prefix crash scenario also runs. These are process
crash tests, not power-loss or filesystem-full guarantees. No universal maximum
crash-loss bound is claimed.

## Harness cleanup and verification

The prior teardown read only two 4096-byte blocks, which could not consume a
large framed greeting, then killed a text-matched supervisor while leaving its
child behind. It now reads complete frames capped at 4MiB, handles controller
EOF/refusal, waits for explicit shutdown completion and endpoint/authenticated
peer exit, and releases only its own isolated storage fault markers. Exceptional
recovery cleanup targets the socket-authenticated peer and an observed child
still parented by it, rather than a command-substring process scan. The outer
reader uses `sync_channel(8)` and owned-dashboard kill drains it while waiting.
The ticket-13 retrospective is corrected separately without rewriting raw records.

Targeted fault checks passed seven scenarios, including the retained real final
SQLite-commit failure. Additional targeted existing-child, native-tool fallback
and survivor checks passed after their protocol/readiness corrections. The first
combined run passed 36/39 ownership scenarios and exposed the existing-child
refusal ordering, fallback before old history drained, and an overly broad
cancellation clear; those were corrected and their focused regressions passed.
A second combined run on recorded source `ccf2cb5` passed locked build,
format, warnings-denied all-target Clippy, 22 unit checks (plus the ignored
subprocess probe exercised by the passing terminal driver), all 39 ownership
scenarios, the scripted-session scenario (26.61s), and the screen scenario (8.13s).
The 39-scenario ownership portion completed in 239.62s, including the sustained
bounded-harness resource workload.

A final warning-only change at `f9c62bd` moves the retained possible-loss message
above an active storage error. Its new render-seam test first failed because the
message was hidden and then passed with both the warning and y retry visible.
Locked build, formatting and warnings-denied all-target Clippy pass on that
source. The preceding aggregate receipt does not pretend its old binary tested
this final warning placement. The optional exact-source repeat and its later correction are recorded below.

Reproduction:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
# Fresh path required; never overwrite a recorded run:
RAT_STORAGE_EVIDENCE=/private/tmp/new-storage14.jsonl cargo test --locked --test ownership storage_saturation -- --nocapture
```


### Provisional receipt-gap follow-up

The additional exact-source repeat passed the sustained resource workload but
finished 38/39 ownership scenarios: lost-launch-ack recovery intermittently saw
`launch_result=null` with `metadata_pending=false`. Authorize had committed, but
the native exec receipt/result Save had not yet arrived. This was a real pending
state gap, not evidence of a failed durable write. A deterministic real-process
regression now holds the exec receipt on its existing worker through the private
debug `proof-launch-receipt` marker. It first failed with precisely that false
pending flag. The aggregate pending predicate now includes pending launch,
receipt and outcome phases; shutdown also waits for them. The held-receipt
regression passes with provisional state, Stop, cancellation, subsequent Shutdown
and no late execution. It additionally exposed immediate Shutdown mistaking the
current owned stopping worker for restored unknown execution; the check now
distinguishes those cases and retains the conservative restored-state policy.
Marker cleanup releases only this additional fixture-owned hold.

Final follow-up checks passed locked build, format and warnings-denied
all-target Clippy. The deterministic held-receipt test passed in 0.78s. The six
launch-filter scenarios passed together in 14.98s on committed follow-up
`cffd349`: existing-child refusal, launch gate arguments, lost acknowledgement,
held receipt with immediate Shutdown, failed-result cache, and all four crash
windows. No automatically owned coding child appeared in any restored crash row.

The aggregate command `cargo test --locked --all-targets -- --skip
sustained_capture_crosses_caps_while_attached_focused_detached_and_lagged` passed
23 unit checks (one intentionally ignored probe driven by terminal restoration),
39 ownership checks (the resource scenario explicitly filtered), scripted
interaction in 26.64s and screen integration in 8.34s. The ownership portion took
84.52s. This aggregate occurred during the follow-up diagnosis/build series;
the separately completed six-scenario run is the precise committed-code receipt
for the changed control boundary. The preceding full runs already exercised the
unchanged sustained resource workload successfully; they do not falsely claim
that final receipt-state changes had an additional full resource run. Ticket 18
retains the final whole-union all-target gate.

The branch merged integration `13466db` before its main checks and latest
integration `39d0ed8` before final handoff, retaining the native classifier edits.
Two old failed ticket-14 fixtures remained from before the cleanup correction:
peer 83196 at its exact isolated state endpoint acknowledged authenticated
Shutdown; orphan fixture 84815 was identified by exact PID, parent, start timestamp
and argv before terminating its own group. No substring process targeting or
normal user endpoint was used. A final read-only inventory found **no** ticket-14
test, supervisor or fixture processes remaining. The current bounded teardown
was exercised by all subsequent fault/control runs.
