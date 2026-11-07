# 09: Resolve repository-first review guidance

Status: ready-for-agent
Resolved: no
Blocked by: 07

**What to build:** Show which instructions will guide a review: applicable target-branch repository guidance first, otherwise a bundled review skill or configured custom fallback, with actionable failures and recorded provenance.

**Acceptance coverage:** R3 repository context and guidance; R10 actionable guidance failures.

## Acceptance criteria

- [ ] Discover applicable repository review instructions and review skills from the pinned target-branch revision. Resolve relevant referenced guidance and general repository conventions using explainable precedence.
- [ ] Show new or changed instructions in the PR as proposed guidance rather than silently allowing them to govern their own review.
- [ ] When repository review guidance is absent, use a versioned bundled review-focused adaptation of Matt Pocock code-review or an explicitly configured custom fallback. Do not install the full collection into coding sessions.
- [ ] Adapt the fallback for unattended inspection: repository standards and available originating requirements, honest missing-spec evidence and structured findings. Do not require a user interview or assume parallel subagents work unchanged.
- [ ] Existing but unusable guidance shows the missing dependency/error and offers Retry or explicit Use fallback. Readable instructions requiring forbidden execution are retained with those steps reported unverified.
- [ ] Expose the resolved guidance and exact provenance/version identity in the review context, and make it available for preparation records. Changing fallback configuration cannot start usage or rewrite existing drafts.
- [ ] Verify absent, repository-specific, referenced, malformed and custom guidance, proposed instruction changes and unavailable execution requirements through the workflow seam; persist configuration/provenance needed by this working slice.

## Scope and handoff

Provide a working guidance inspection/recovery flow rather than a standalone parser. Ticket 12 exposes the full setup/defaults experience; ticket 10 consumes resolved guidance under enforced reviewer restrictions.

## Comments
