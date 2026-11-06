# 13: Bound output delivery to slow dashboards

Status: ready-for-agent
Resolved: yes
Blocked by: 04, 07, 08

**What to build:** Keep navigating and reconnecting during sustained output, even when dashboard delivery is slow or fully detached, without unbounded memory or dropped transcript data.

- [x] Declare concrete queue, screen/scrollback, chunk and in-flight persistence caps before the workload. Separate bounded screen delivery from durable transcript capture so slow subscribers cannot grow memory indefinitely.
- [x] When dashboard delivery falls behind, use an explicit bounded resynchronization policy and recover a coherent snapshot plus subsequent output without transcript gaps or duplication.
- [x] Exercise attached, dashboard-focused, fully detached and slow-subscriber runs beyond queue saturation and multiple transcript chunks. The child remains drained subject to explicit capture backpressure.
- [x] Observe queue high-water marks, screen bounds and dashboard/supervisor memory separately. Demonstrate bounded behavior rather than growth proportional to accumulated history; record achieved output rate and producer blocking.
- [x] Navigation, refresh, resize and reconnect remain usable under the fixture workload with bounded per-frame event work. Capture preliminary visible-feedback and attach timings for later repeatable measurement.
- [x] The integration harness asserts transcript continuity and bounded queues through lag and reconnection. Document caps and delivery/backpressure policy without promising a complete product capacity budget.


## Comments

Implemented and verified against the declared fixture workload on 2026-11-06.
The supervisor retains a single bounded partial-write cursor, does at most 64KiB
nonblocking transport work per pass, and disconnects a stalled frame after 500ms
with explicit resync guidance. Reopening recovers the same identity and coherent
screen; raw capture remains lossless and independently backpressured. Typed
connection loss preserves attribution, labels process state unknown and supports
local `d` detach/reopen. OSC parsing now has a fixed 1024-byte payload cap, with a
real 8MiB red/green memory/capture regression. Capture and subscriber high-water
observations, producer write waits, and separate process RSS are retained.

See [caps, TDD, limitations, and final verification](../../../docs/proofs/13-slow-subscribers.md)
and its three immutable raw JSONL records. Final combined source be1b40b includes
integration 0c8e1b8 (tickets 12/16); stable locked build, formatting,
warnings-denied all-target Clippy and all-target tests passed: 22 unit tests,
33 ownership/history/recovery scenarios, scripted interaction and screen tests;
one ignored subprocess probe is covered by its passing restoration driver.

Each of four modes emits 4MiB, exactly 16 configured capture-queue capacities,
with real queue saturation and at least 64 transcript chunks per mode. All
131072 records are recovered exactly once. This resolves bounded delivery and
capture behavior for that workload, not a complete product capacity budget or
long-duration plateau proof. Active-burst usable attach remained above one
second (final 1.482781s); timestamps show 1.411855s before the synchronous fixture
could parse input and 70.942ms from parse to visible ACK. Ticket 17 must use a
paced producer with independent input handling and repeatable long measurements.
Storage failures, partial persistence writes and full-disk recovery remain ticket
14; no such gate is claimed passed here.
