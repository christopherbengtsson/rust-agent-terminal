# 06: Discover personal requests in the Reviews inbox

Status: ready-for-agent
Resolved: no
Blocked by: 04

**What to build:** Let the developer discover and revisit personal GitHub review requests in a keyboard-driven Reviews inbox, including repositories that are not registered local projects.

**Acceptance coverage:** R1 discovery, R8 request identity foundation, R9 eligibility.

## Acceptance criteria

- [ ] Use the proven request discovery semantics and existing GitHub CLI authentication. Missing executable, authentication problems, offline status and stale data produce actionable UI states.
- [ ] Retain repository/PR/request-cycle identity through the sole writer and show discovered requests after restart without requiring a coding workspace or starting any agent.
- [ ] Support bounded background refresh, pagination and keyboard selection. Preserve useful last-known data on failure and target normal new-request discovery within two minutes when GitHub is reachable.
- [ ] Refresh closed, merged or withdrawn eligibility without deleting retained request/draft identity. Treat renewed requests consistently with ticket 04 evidence rather than deduplicating solely by head commit.
- [ ] Discovery and UI navigation perform no repository mutation, agent launch or GitHub write. Unknown remote state remains visibly unknown.
- [ ] Verify the supervisor command/event workflow with controlled authentication/network/request sequences, restart and selection changes; include one real authenticated read of an unregistered repository request where available.
- [ ] Follow the established dashboard composition and small-terminal behavior with honest unavailable states for features not yet implemented; expose no nonfunctional preparation controls.

## Scope and handoff

Add structured records and migration only for this working inbox slice. Own-PR tracking, team-only discovery and macOS notification delivery remain out of scope.

## Comments
