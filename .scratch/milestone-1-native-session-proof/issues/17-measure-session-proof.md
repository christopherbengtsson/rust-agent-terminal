# 17: Measure responsiveness and resource bounds

Status: ready-for-agent
Blocked by: 09, 14

**What to build:** Produce reproducible measurements showing what the session runtime can sustain, which targets it meets, and where faults or resource limits constrain it.

- [ ] Before running, declare hardware, macOS/terminal/Rust versions, build profile, viewport sizes, commands, sample count, duration, byte rate/volume, escape/redraw mix, queue caps, subscriber lag, storage delays and instrumentation overhead.
- [ ] Run attached, dashboard-focused, fully detached, slow-subscriber and slow/full-storage workloads beyond saturation and multiple transcript chunks. Report actual output throughput and blocked-producer behavior.
- [ ] Measure input event to visible navigation feedback while output and refresh run, and attach action to synchronized usable screen/input for an already-running session. Report distributions and worst observations against 100 ms navigation and under-one-second attach targets; exclude fresh launch startup.
- [ ] Report dashboard and supervisor peak/steady-state memory separately, child usage separately, queue high-water marks and screen/scrollback bounds. Demonstrate a bounded plateau and explain allocator/OS overhead or exceeded caps.
- [ ] Measure emitted/captured/durable bytes, transcript and metadata growth, persistence bytes/sec and durable flush intervals against the one-second healthy-storage target. Verify clean-shutdown flush and recovered durable prefix after abrupt failure, explicitly reporting unflushed loss and write failures.
- [ ] Measure fault responsiveness for navigation/attach, retry and stop. Include session/request identities and child counts from reconnect/crash runs so performance results do not conceal duplicate coding.
- [ ] Publish reproducible commands, raw results and a concise pass/fail record with capacity limits. Missed latency/durability targets, silent loss or unbounded accumulation remain open proof failures with proposed resolutions; do not invent product memory/disk promises or claim the full release workload.
