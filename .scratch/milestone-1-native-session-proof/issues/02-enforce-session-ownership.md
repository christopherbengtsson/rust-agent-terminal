# 02: Enforce supervisor and worktree ownership

Status: ready-for-agent
Blocked by: 01

**What to build:** Give a developer one controlling dashboard and at most one managed active session per worktree, even when startup races or checkout aliases are involved.

- [ ] Concurrent supervisor startups elect one user-wide owner; the local endpoint is user-private. Recover stale endpoints without displacing a live supervisor or allowing competing session owners.
- [ ] A second controlling dashboard explains the existing connection and cannot send input, resize, launch or shutdown commands. Detect a dead client so a later dashboard can acquire control.
- [ ] Canonical worktree identity unifies supported path aliases. Repeated launch attempts and aliases cannot start a second active managed session in the same checkout; attaching selects the existing session.
- [ ] The UI identifies the selected checkout and explains ownership conflicts without losing the last successful worktree discovery or freezing navigation.
- [ ] Integration scenarios cover simultaneous startup, stale sockets, dead clients, second-dashboard refusal, repeated launch and alias paths in real temporary repositories. Assert observable owner and child counts rather than private routing details.
- [ ] Document ownership and endpoint recovery behavior in line with the accepted user-wide-supervisor decision; unresolved ownership must fail closed rather than permit duplicate coding.
