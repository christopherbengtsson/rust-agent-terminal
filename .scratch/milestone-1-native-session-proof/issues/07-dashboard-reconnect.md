# 07: Reconnect after dashboard loss

Status: ready-for-agent
Blocked by: 02, 06

**What to build:** Reopen after dashboard loss and continue the same surviving session without replaying a launch or duplicating coding activity.

- [ ] Kill the dashboard or close its terminal while the child emits output. Reopen, acquire the released controlling connection and recover the same supervisor/session identity and usable current screen.
- [ ] Reconnect obtains a consistent snapshot and ongoing output/input from that child; navigation and resize remain usable without a replacement launch.
- [ ] A stable launch-request identity survives the client recovery path. Drop the acknowledgement after the supervisor has processed launch, then query/retry that same request and recover its original result.
- [ ] Same-request retries during connection loss do not start another child. Return recorded launch failure as failure rather than silently issuing a new launch.
- [ ] The isolated integration harness asserts identities, observable child count, output continuity and takeover after client death, including a slow or lost response. A live controlling dashboard still excludes a second one.
- [ ] Document reconnect behavior and the distinction between dashboard loss and supervisor loss. Cross-supervisor crash reconciliation is explicitly deferred to the uncertain-launch ticket.
