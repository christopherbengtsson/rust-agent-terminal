# 17: Measure responsiveness and resource bounds

Status: ready-for-agent
Blocked by: 09, 14
Resolved: yes

**What to build:** Produce reproducible measurements showing what the session runtime can sustain, which targets it meets, and where faults or resource limits constrain it.

- [x] Before running, declare hardware, macOS/terminal/Rust versions, build profile, viewport sizes, commands, sample count, duration, byte rate/volume, escape/redraw mix, queue caps, subscriber lag, storage delays and instrumentation overhead.
- [x] Run attached, dashboard-focused, fully detached, slow-subscriber and slow/full-storage workloads beyond saturation and multiple transcript chunks. Report actual output throughput and blocked-producer behavior.
- [x] Measure input event to visible navigation feedback while output and refresh run, and attach action to synchronized usable screen/input for an already-running session. Report distributions and worst observations against 100 ms navigation and under-one-second attach targets; exclude fresh launch startup.
- [x] Report dashboard and supervisor peak/steady-state memory separately, child usage separately, queue high-water marks and screen/scrollback bounds. Demonstrate a bounded plateau and explain allocator/OS overhead or exceeded caps.
- [x] Measure emitted/captured/durable bytes, transcript and metadata growth, persistence bytes/sec and durable flush intervals against the one-second healthy-storage target. Verify clean-shutdown flush and recovered durable prefix after abrupt failure, explicitly reporting unflushed loss and write failures.
- [x] Measure fault responsiveness for navigation/attach, retry and stop. Include session/request identities and child counts from reconnect/crash runs so performance results do not conceal duplicate coding.
- [x] Publish reproducible commands, raw results and a concise pass/fail record with capacity limits. Missed latency/durability targets, silent loss or unbounded accumulation remain open proof failures with proposed resolutions; do not invent product memory/disk promises or claim the full release workload.

## Evidence

Resolved by the [recorded measurement proof](../../../docs/proofs/17-session-measurements.md)
and standalone main/supplement raw archives. Clean source `b2a43e1` passed the
438.28s declared sustained run: all 200 navigation samples and all 100 attaches
were entirely inside active production, worst 8.375ms and 661.666ms respectively.
All four healthy storage modes met the one-second measured flush/age target.
422 process-attributed RSS samples show later steady plateaus while 311 chunks
retain 20,347,181 accepted/durable bytes. No product memory/disk budget or full
release workload is claimed.

A separately declared 34.03s full-storage receipt supplement at `bc56e56` passed
with the same production runtime prefix and a corrected scripted input reader:
receipt 661.762ms, ACK right-censored beyond two seconds until repair, then the
same ACK visible 124.225ms after repair/retry. Full-storage native output remains
a documented backpressure limit, not a healthy attach pass or full usability
claim. Stop was observed before repair in the sustained run.

Accepted/written/durable prefixes match after clean shutdown. The paced crash
restored the exact acknowledged 120,988-byte prefix without automatic coding; at
least 4096 accepted but unflushed bytes were lost. Exact accepted-at-kill loss is
unknown across the recorded counter/kill sampling gap. Earlier EPERM Stop and
harness/input-fixture mistakes remain in archived failure records with their
causal fixes; no failed or incomplete run is silently promoted to a pass. Final
merged all-target/native validation remains ticket 18.
