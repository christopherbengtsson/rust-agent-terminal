# 02: Enforce supervisor and worktree ownership

Status: ready-for-agent
Blocked by: 01
Resolved: yes

**What to build:** Give a developer one controlling dashboard and at most one managed active session per worktree, even when startup races or checkout aliases are involved.

- [x] Concurrent supervisor startups elect one user-wide owner; the local endpoint is user-private. Recover stale endpoints without displacing a live supervisor or allowing competing session owners.
- [x] A second controlling dashboard explains the existing connection and cannot send input, resize, launch or shutdown commands. Detect a dead client so a later dashboard can acquire control.
- [x] Canonical worktree identity unifies supported path aliases. Repeated launch attempts and aliases cannot start a second active managed session in the same checkout; attaching selects the existing session.
- [x] The UI identifies the selected checkout and explains ownership conflicts without losing the last successful worktree discovery or freezing navigation.
- [x] Integration scenarios cover simultaneous startup, stale sockets, dead clients, second-dashboard refusal, repeated launch and alias paths in real temporary repositories. Assert observable owner and child counts rather than private routing details.
- [x] Document ownership and endpoint recovery behavior in line with the accepted user-wide-supervisor decision; unresolved ownership must fail closed rather than permit duplicate coding.

## Comments

Implemented the user-private OS ownership lock, stale/refused socket recovery,
controller handshake/refusal and dead-client release, separate supervisor OS
session, and canonical Git worktree adoption. The real-dashboard/supervisor/PTY
scenarios establish one owner/child under racing starts, refused second control,
same-child reconnect, aliases and repeated launch; errors preserve discovery and
navigation. See [recorded evidence](../../../docs/proofs/02-session-ownership.md).
Stable locked build, formatting, warnings-denied Clippy and all-target tests pass
on macOS 26.2 arm64. Socket/PTY tests used approved execution outside the sandbox.
No native-tool support or supervisor-crash survivor reconciliation is claimed;
those remain later proof tickets.
