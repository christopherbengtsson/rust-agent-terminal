# Paced session responsiveness and resource measurements

This proof uses the approved real dashboard/supervisor/scripted PTY seam on
macOS. The external terminal is a continuously drained portable-pty byte stream
parsed by vt100 0.18.1 at 160×40, with zero scrollback. Timings end at the first
complete parsed visible screen; they do not timestamp physical monitor pixels.
Native Claude/Codex compatibility remains the separate versioned native proof.

The final manifest is the first line of `observations.jsonl` in the final raw
archive. It precedes production and includes source revision/dirty state, binary
SHA256, hardware/OS/Rust/profile, geometry, rates, duration, sample counts, caps,
style mix, fault delays and instrumentation. The attached phase lasts 90 seconds; dashboard focused, fully detached, slow
subscriber and slow-storage phases last 60 seconds each. They offer 64KiB/s
(attached, dashboard focused, fully detached, slow subscriber) or 256KiB/s with
50ms per-append delay (slow storage). Full storage uses synthetic debug ENOSPC
before the real append, a 30-second saturated dwell and explicit Stop/retry.
This does not exhaust the user's disk or establish actual volume-full semantics.

Each producer record is exactly 1024 bytes with a numbered run/sequence, cursor
position/erase, green SGR, lambda/CJK UTF-8 and X padding. One finite producer
paces outside its short stdout lock; the main thread continues reading input.
There is no catch-up burst following backpressure. Unique ACK receipts are
timestamped before stdout acquisition; their visible delivery can be blocked by
actual PTY/storage backpressure. A bounded ACK on a frozen current view is not
reported as full interactive usability.

Normal capture owns at most 270336 pending raw bytes. Explicit Stop additionally
reserves a finite FIFO of at most 256 blocks/1MiB to permit terminal exit drain;
the conservative combined raw ceiling is 1318912 bytes, plus fixed stack storage
and bounded descriptors/session IDs. Exhaustion still blocks and can leave Stop
unconfirmed. Other limits are 64 capture slots, 16 storage commands/events,
32 native-input/client requests, one replacement client event, one outbound
frame capped at 4MiB, 8×4096-byte outer blocks, two at-most-200×80 child screens,
zero scrollback, 1024-byte OSC payload and 64KiB transcript chunks. These are
component bounds, not total-process RSS or total retained-disk promises.

The 1024-entry commit ring records every successful data-bearing transcript
`File::sync_all` followed by SQLite FULL transaction commit (SQLite fullfsync
concerns its own files). Sequence gaps invalidate a complete distribution;
last-counter polling is insufficient. Failed appends no longer mint repeated
unchanged-prefix completions. Oldest accepted age includes capture waiting for
the committed batch, not a per-byte exhaustive latency trace. Idle/no-data gaps
and injected faults are classified separately from healthy active intervals.
Process-crash recovery does not prove sudden-power-loss safety.

RSS samples identify harness, dashboard, supervisor and child separately and
record actual `ps` cost. First/middle/last steady windows exclude five seconds of
warmup. RSS is sampled resident memory, not an instantaneous allocation peak.
Disk samples distinguish logical transcript/SQLite sizes and transcript allocated
blocks. Terminal parsing and producer-ledger costs are recorded separately; the
no-output navigation baseline includes harness/application scheduling overhead.
The sampled workload does not establish the full release workload or a numeric
product memory/disk budget.

Reproduce serially with no other Cargo/native workload running:

```sh
cargo build --locked
RAT_MEASURE17_DIR=/private/tmp/unique-new-final-run cargo test --locked --test ownership paced_session_measurement17 -- --exact --ignored --nocapture
python3 docs/proofs/17-analyze.py /private/tmp/unique-new-final-run
```

`RAT_MEASURE17_QUICK=1` selects four-second/5-attach/10-navigation preflight
parameters. Those runs debug the harness; they cannot demonstrate a sustained
memory plateau or substitute for the final declared sample set.

The harness checks representative actual public History pages at the beginning,
a rotation boundary and the final prefix. Its crash scenario additionally opens
retained History in the dashboard, checks the same identity and no automatic
coding child, and refuses a replacement while execution remains uncertain.
Additional read-only SQLite/chunk forensics stream only committed extents, check
all numbered records in order without a growing record list, hash the actual raw
prefix and compare it before/after restore. These forensics supplement the real
History assertions. Clean shutdown requires its actual final acknowledgement and
endpoint removal before artifact collection.

Historical failure evidence is retained in
`17-preflight-stop-failure.tar.gz`. Preflight 04 reproduced StopFailed/EPERM with
the child in macOS `?Es` and 67584 accepted bytes pending. Repair/retry then
confirmed Stop. The [causal bounded-drain proof](17-stop-drain.md) records the
minimized red/green check. Its earlier duration/flush figures predate the dirty
append correction and must not be used as the final durable distribution.

Final results are recorded after the declared full run; this protocol alone
closes no measurement gate.
