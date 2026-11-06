# 07: Reconnect after dashboard loss

Status: ready-for-agent
Blocked by: 02, 06
Resolved: yes

**What to build:** Reopen after dashboard loss and continue the same surviving session without replaying a launch or duplicating coding activity.

- [x] Kill the dashboard or close its terminal while the child emits output. Reopen, acquire the released controlling connection and recover the same supervisor/session identity and usable current screen.
- [x] Reconnect obtains a consistent snapshot and ongoing output/input from that child; navigation and resize remain usable without a replacement launch.
- [x] A stable launch-request identity survives the client recovery path. Drop the acknowledgement after the supervisor has processed launch, then query/retry that same request and recover its original result.
- [x] Same-request retries during connection loss do not start another child. Return recorded launch failure as failure rather than silently issuing a new launch.
- [x] The isolated integration harness asserts identities, observable child count, output continuity and takeover after client death, including a slow or lost response. A live controlling dashboard still excludes a second one.
- [x] Document reconnect behavior and the distinction between dashboard loss and supervisor loss. Cross-supervisor crash reconciliation is explicitly deferred to the uncertain-launch ticket.


## Comments

Implemented and verified on macOS through isolated real dashboard/supervisor/PTY
scenarios. Reconnect retains session/child identity, styled sequenced output,
input and resize. Lost success and failure acknowledgements recover by stable
request identity; same-request retries never launch again. Control observations
are reliable while screen-only observations coalesce. See
[proof and reproduction](../../../docs/proofs/07-dashboard-reconnect.md).
Owner-lifetime only: durable metadata is ticket 08; cross-supervisor uncertainty
and surviving children remain ticket 09.
