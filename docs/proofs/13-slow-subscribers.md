# Ticket 13: bounded subscriber proof

Predeclared workload and caps (before resource runs): stable Rust debug build,
real temporary Git checkouts, real dashboard/supervisor/scripted PTYs on macOS.
Four modes: attached, dashboard focused, entirely detached, and a subscriber
whose actual socket is not read. Each emits 32768 numbered 128-byte records
(4 MiB), equal to 16 configured capture-queue capacities (15.5 times the combined
pending-byte cap) and at least 64 transcript chunks. The
preliminary harness samples each PID's RSS separately; disk history grows.

Caps: two 200x80 screens, zero scrollback, a fixed 1024-byte OSC parser
payload, 4096-byte PTY reads, 64 capture blocks,
one worker-owned block plus one producer block awaiting enqueue (270336 pending
owned bytes), plus the reader's 4096-byte stack buffer. At most one 65536-byte
raw chunk is open at a time; writes/sync/SQLite commit run in the sole storage
worker. Capture blocks wait on a full queue without discarding raw bytes; a
blocked-output child can still do other work. Input queue: 32 x 4096 bytes;
transport request queue: 32 bounded requests; observation queue: one finite
snapshot. Protocol frame/request buffer: 4 MiB; pending request accumulation can
overshoot by one 4096-byte read before rejection. Metadata/completion queues
have 16 slots; history pages use 2048 raw bytes with finite escape expansion.
Native status retains 16 tools and 16 summaries and reads <=32 receipts/pass.

Subscriber responses retain a single complete JSON frame and cursor, with
<=64KiB nonblocking writes per supervisor pass. A 500ms frame deadline closes
the socket and retains an explicit slow-dashboard reason for reconnect. Partial
frames are never followed by another frame on that socket. Other clients remain
refused promptly while control is held. New control after disconnect receives a
complete current screen and its owner-lifetime parsed-byte boundary; snapshots
replace the previous display rather than replay incremental output. Raw capture
is independently queued to persistence and never reconstructed from snapshots.
The UI processes at most one observation plus 32 Git events/frame and renders
only the bounded current screen.

Connection loss revokes semantic confidence and displays unknown process state
with last-seen lifecycle and retained identity. Press d to detach locally, then
reopen the same state directory; no launch is performed. q requires an actual
supervisor stop/flush acknowledgement and cannot falsely confirm shutdown.

This is preliminary scripted evidence, not native-agent support, a complete
capacity budget, storage fault proof (ticket 14), power-loss proof, or ticket
17's repeated measurement distributions. Terminal-parser visible timing is not
physical-pixel presentation timing.

## Reproduction and test seams

The TDD seams remain the approved real dashboard/supervisor/scripted PTY and
App/render interfaces. The first slow-subscriber test failed because the old
nonblocking `write_all` disconnected without an explicit resync reason; the
partial-frame cursor made it pass. The connection-loss App test first failed
to compile before typed loss observations existed. The sustained test first
failed because the fixture did not implement the numbered burst.

A real nonreading socket receives a maximum 200x80 styled snapshot. While it is
stalled, a competing connection receives its ownership refusal within a 300ms
asserted budget, before the 500ms deadline; after deadline reconnect gets the
same identity, coherent final screen and explicit reason. A separate real
dashboard test pauses only its isolated supervisor, observes a real transport
timeout, retains attribution and last-seen lifecycle, exits with local `d`, then
reopens against the resumed owner and confirms the original child PID and input.
No automatic coding launch occurs.

The harness outer-output channel is bounded to eight 4096-byte blocks. This
exposed macOS PTY cleanup behavior: portable-pty's Unix `kill` sends SIGHUP, and
even a SIGKILL can await a blocked tty write. Cleanup now drains the bounded
outer channel while polling the owned fixture's exit, with a five-second bound.
The existing concurrent-dashboard test reproduced the stall and then passed in
1.74s after this correction. Only known isolated failed proof processes were
stopped; no normal user endpoint was connected or modified.

Use a fresh output path: resource artifacts use `create_new` and never overwrite
an earlier run. Omitting the environment variable uses an automatically removed
fixture directory.

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
evidence=$(mktemp -d /private/tmp/rat13-evidence.XXXXXX)
RAT_RESOURCE_EVIDENCE="$evidence/resource.jsonl" cargo test --locked --all-targets -- --nocapture
```

The immutable [preliminary raw record](13-resource-preliminary.jsonl) identifies
clean source `95e238db1415ab2b5beceda36d9868df0ba19e0b`. Its resource scenario
passed in 140.46s, although that full suite had one diagnostic-layout regression:
a long connection-loss prefix wrapped the existing private-directory error phrase.
The error is now presented first, and its targeted real-dashboard test passed.
The later confirmation record includes a fixture command-parsed timestamp for
separating burst/input delay from terminal delivery.

## Preliminary observations and limits

macOS 26.2 (25C56), Darwin 25.2.0 arm64, Mac16,8, 48GiB RAM, 12 logical CPUs;
stable rustc 1.99.1, debug profile. The outer terminal is the harness's vt100
parser at 160x40, not a graphical terminal. These short runs overlap other
repository proof work; they are not an isolated performance benchmark.

The first record contains 131072 numbered records exactly once through public
readable-history pagination. Captured and acknowledged current-session raw prefix
both reached 17381092 bytes, including startup, controls, mosaic and statistics.
This is the first session in that owner, so its lifetime parser offset and current
session durable prefix are comparable without subtracting an earlier session.
Four bursts emitted 16MiB plus a separate maximum-style mosaic. Disk history is
retained and grows; it is not counted as bounded total disk use.

The first run observed 67584 pending raw bytes, 267 actual full-queue waits
(totalling 3.581460s) and a 359011-byte serialized subscriber frame high-water.
Mac PTY reads were often smaller than 4096 bytes: filling all 64 slots need not
reach the worst-case 270336 pending-byte cap. Pending bytes returned to zero.
Fixture successful-write rates were 2.689/2.111/2.303/2.459MiB/s for attached,
dashboard focused, fully detached and slow subscriber respectively. Their longest
individual writes took 29.695/47.653/45.518/73.805ms. Offered output was unpaced;
these are achieved successful-write rates, not promised product throughput.

First-run sampled RSS ranges (KiB) during the short output phases:

| Mode | Dashboard | Supervisor | Child |
| --- | ---: | ---: | ---: |
| Attached | 6128–6624 | 6336–6496 | 2480–2864 |
| Dashboard focused | 6624–6656 | 6496–6512 | 2944 |
| Fully detached interval | absent after exit | 6512 | 2992 |
| Slow subscriber | absent | 6512–8320 | 2992–3024 |

A sampler row near dashboard exit still includes its process; the raw rows retain
PID, PPID, command and elapsed lifetime for attribution. The sampler waits 100ms
*after* `ps` completes; its own median cost was 82.183ms, maximum 193.266ms.
Thus these are sparse sampled residents, not exact allocation counts or true
instantaneous peaks. The supervisor later reached 8560KiB while paging history.
Its rise includes changing from the dashboard viewport to maximum screen geometry;
this short observation does not establish a long-duration memory plateau or the
complete product workload budget. The retained raw transcript grew by over 16MiB
without a matching in-memory transcript accumulation.

CLOCK_MONOTONIC timestamps are shared across the fixture and input collector;
the resource sampler's elapsed times use the collector's own origin. Of ten
preliminary visible navigation samples, the first three injections preceded the
second burst's actual producer start, so they are excluded from the active-output
summary. Seven active injections measured 3.993–8.069ms (median 4.257ms) to a new
parsed Selection branch. Refresh keys were also sent. This small sample is not a
latency distribution or physical-pixel evidence.

The active-burst attach injection produced a focus frame after 7.686ms but the
unique fixture nonce ACK became visible only after 1.917265s: **the conservative
usable-input metric missed the one-second target**. The focus frame is not an
attach acceptance result. This fixture emits the entire burst synchronously on
its input-reading thread; it cannot parse the queued nonce until output finishes.
The confirmation run records the nonce's command-parsed monotonic timestamp
before writing its ACK so this delay can be separated from final visible output.
Ticket 17 must use a longer paced producer with independent input handling and
repeat distributions, plus appropriate physical-terminal evidence. Neither this
fixture artifact nor the numeric focus time closes that target.

Resize/coherent maximum screen, native input, focus, refresh, reconnect and
bounded per-frame work retain their earlier integration checks. These records
establish saturation, finite capture/subscriber ownership, and transcript
continuity for the declared workload. Storage faults, retry/full-disk behavior
and partial persistence writes remain ticket 14; no such gate is closed here.

## Confirmation and parser control-string bound

The [confirmation raw record](13-resource-confirmation.jsonl) identifies clean
source `6da639d9a6eb8b1aa061b8d7dcca4dd5b8c2d1c0`. Its resource scenario passed
in 140.42s. That all-target run passed 21 unit tests (one ignored probe exercised
by the restoration driver), 29 ownership scenarios in 187.72s, scripted interaction
in 14.66s, and screen integration in 8.65s. Build/fmt/warnings-denied Clippy passed.
Its pending high-water was again 67584 bytes, with 260 full-queue waits totaling
2.868250s, a 359007-byte frame high-water and 17381130 accepted/durable raw bytes.
The 38 additional bytes are the fixture input timestamp. All 131072 numbered
records remained continuous and unique.

Eight confirmation navigation injections occurred during actual production;
their terminal-parsed visible times were 3.930–8.004ms (median 5.889ms). Two were
excluded because their injections preceded the burst. The active attach's ACK
took 1.537147s, again missing the one-second target. The fixture parsed its nonce
1.430444s after injection and only 0.169ms after finishing the burst. The remaining
106.717ms was fixture-parse to terminal-parsed visible ACK. This supports the
synchronous fixture input-loop diagnosis rather than attributing the whole delay
to product rendering; it does not waive the miss or prove the product tail meets
all targets. Raw start, finish, injected, parsed and visible timestamps use the
same kernel CLOCK_MONOTONIC. The final paced/concurrent-input measurement remains
open in ticket 17.

The final cap review found an additional real parser risk: vt100's default `std`
feature enables vte's growable OSC payload Vec even with zero screen scrollback.
It is now disabled; vte uses its fixed 1024-byte OSC array. Only vt100's unused
`std::io::Write` convenience implementation is removed; product parsing uses
`process()`. `cargo tree --locked -e features -i vte` confirms no `vte/std` feature.
Oversized OSC payloads are truncated only inside the live terminal parser. Window
names/clipboard controls remain unsupported; the independent raw transcript
retains all bytes.

The real supervisor regression emits one 8MiB OSC title control plus its terminator
and a visible final marker, then observes its screen, parsed boundary, durable
prefix and public escaped history. Before the configuration change the test
failed: supervisor RSS grew from 6336 to 16816KiB. With the fixed OSC parser it
passed in 4.28s: 6368 to 6528KiB, with exactly 8388638 additional parsed/durable
bytes and the escaped OSC prefix readable through history. Its 4096KiB RSS-growth
assertion is deliberately a generous fixture regression gate, not an absolute
product RSS promise. These raw observations were emitted by the real test:

```text
before: OSC13 supervisor RSS before/after: 6336/16816 KiB, captured8MiB control
before: FAILED — live OSC parser retained payload-sized memory
fixed:  OSC13 supervisor RSS before/after: 6368/6528 KiB, captured8MiB control
fixed:  oversized_osc_is_captured_without_growing_live_screen_history ... ok
```

## Final combined verification

[Final bounded-parser raw record](13-resource-bounded-parser.jsonl) identifies
clean combined source `be1b40bfaa08484b6d96505b7c6ff2e2ba9b1921`: ticket 12's
Codex status, ticket 16's native recovery, and ticket 13's bounded OSC/parser,
frame cursor, capture telemetry and harness cleanup. The integration head
`0c8e1b82553716d667a7be10eac17dae6046ca28` was merged before this run.

All required checks passed on that combined source:

```text
cargo build --locked: passed
cargo fmt --all -- --check: passed
cargo clippy --locked --all-targets -- -D warnings: passed
cargo test --locked --all-targets -- --nocapture:
  unit: 22 passed; 1 ignored subprocess probe covered by restoration driver
  ownership/history/recovery: 33 passed; 197.55s
  scripted interaction/status: 1 passed; 14.50s
  screen/resize: 1 passed; 8.18s
  maximum 200x80 styled snapshot: 359026 encoded bytes
  OSC13 RSS before/after: 6288/6496 KiB; 8388638 bytes captured exactly
```

The final four-mode scenario passed in 140.34s with all 131072 numbered records
continuous and unique. Pending raw capture returned to zero; its high-water was
67584 bytes, with 257 full-queue waits totalling 2.666302s. The subscriber frame
high-water was 359034 bytes, and parsed/acknowledged raw prefixes both reached
17381130 bytes. These count/byte caps remained fixed while history grew.

Individual sampled RSS ranges were dashboard 6128–6688KiB (18 samples), supervisor
6288–8848KiB (766 samples), and child 2448–3024KiB (53 samples). The dashboard was
absent for detached/slow-subscriber intervals and the child exited before history
pagination. Sampler median/max cost was 78.810/128.845ms, plus its 100ms sleep.
These short sampled observations retain the preliminary limitations above.

Eight final navigation injections occurred during actual production and measured
4.135–8.007ms to a new parsed selection (median 5.919ms); two earlier injections
were excluded. Busy attach's focus frame was 7.798ms, but usable nonce ACK was
1.482781s, again a one-second-target miss. Injection to fixture parse took
1.411855s; parse to visible ACK took 70.942ms. Fixture parsing occurred only 11µs
after its burst finished. This confirms the fixture's synchronous input-loop
delay while retaining the actual miss and the separate terminal-delivery tail.
Ticket 17's sustained workload, concurrent-input fixture, distributions and
physical-terminal presentation checks remain open. Ticket 14's storage-fault
work is also separate. No new native CLI support claim is derived from these
scripted resource checks.
