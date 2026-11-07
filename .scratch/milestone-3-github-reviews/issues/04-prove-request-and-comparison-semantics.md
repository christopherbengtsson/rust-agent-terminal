# 04: Establish GitHub request and comparison semantics

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** Produce executable evidence and an implementable read-side strategy for discovering personal requests, recognizing renewed cycles, refreshing eligibility and identifying exactly which PR comparison is being reviewed.

**Acceptance coverage:** Engineering prerequisite for R1, R6, R8 and R9.

## Acceptance criteria

- [ ] Use existing GitHub CLI authentication and authoritative GitHub behavior/documentation to establish personal-request discovery across accessible repositories, including repositories without local registration and pagination.
- [ ] Distinguish personal requests from team-only requests and establish closed, merged and withdrawn eligibility. Record freshness, authentication failure, offline behavior and applicable rate/query constraints.
- [ ] Establish how a renewed request can be identified even when the PR head and comparison are unchanged. Exercise request removal/re-addition between discovery observations; report any observation gap rather than assuming polling proves event identity.
- [ ] Define and prove head revision, target-branch revision, merge-base/comparison identity and the exact reviewed diff semantics. Cover target-branch movement with unchanged author head, including movement that does and does not change patch content.
- [ ] Use real temporary Git repositories for comparisons, renames and force-push/base-change cases, plus controlled GitHub responses and authenticated read evidence where available. Never mix unrelated working changes into pinned review context.
- [ ] Describe how repository and PR identity survive renamed repositories or changing branch labels, and how a refresh race is represented without claiming an atomic GitHub snapshot.
- [ ] Retain reproducible findings and a concrete read-side contract for downstream tickets. If reliable request-cycle identity or required comparison behavior cannot be established, return for a decision instead of silently narrowing the requirement.

## Scope and handoff

This proof has no dashboard prerequisite and performs no GitHub writes. Its results feed discovery and pinned comparisons; it must not turn into a generic GitHub framework.

## Comments
