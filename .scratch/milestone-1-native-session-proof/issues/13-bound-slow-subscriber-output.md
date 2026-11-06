# 13: Bound output delivery to slow dashboards

Status: ready-for-agent
Blocked by: 04, 07, 08

**What to build:** Keep navigating and reconnecting during sustained output, even when dashboard delivery is slow or fully detached, without unbounded memory or dropped transcript data.

- [ ] Declare concrete queue, screen/scrollback, chunk and in-flight persistence caps before the workload. Separate bounded screen delivery from durable transcript capture so slow subscribers cannot grow memory indefinitely.
- [ ] When dashboard delivery falls behind, use an explicit bounded resynchronization policy and recover a coherent snapshot plus subsequent output without transcript gaps or duplication.
- [ ] Exercise attached, dashboard-focused, fully detached and slow-subscriber runs beyond queue saturation and multiple transcript chunks. The child remains drained subject to explicit capture backpressure.
- [ ] Observe queue high-water marks, screen bounds and dashboard/supervisor memory separately. Demonstrate bounded behavior rather than growth proportional to accumulated history; record achieved output rate and producer blocking.
- [ ] Navigation, refresh, resize and reconnect remain usable under the fixture workload with bounded per-frame event work. Capture preliminary visible-feedback and attach timings for later repeatable measurement.
- [ ] The integration harness asserts transcript continuity and bounded queues through lag and reconnection. Document caps and delivery/backpressure policy without promising a complete product capacity budget.
