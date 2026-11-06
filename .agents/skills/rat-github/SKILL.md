---
name: rat-github
description: Implement GitHub review discovery, restricted preparation, local drafts, revision revalidation, submission, task PR feedback, or merge workflows in this project.
---

# GitHub review and task PR workflows

Paths are repository-relative. This is product implementation guidance.

1. Identify the affected R1–R11 or C8–C9 transition in
   [PRODUCT.md](../../../PRODUCT.md). Locate its owner and persisted/remote result;
   use the milestones to distinguish planned behavior from working features.
2. Select the branch below. For a CLI/API change, check installed `gh --version`
   and relevant `--help` before selecting flags or JSON fields. For a local
   draft/serialization change, use its local state and persistence path.
3. Verify the changed transition and affected failures. Use recorded responses
   for remote event/retry changes; local-only changes need local checks. Report
   fixture coverage and authorized native/GitHub checks separately.

## Boundaries

- **Execution placement:** when changing `gh`/reviewer execution, authentication
  ownership, diff delivery or request-outcome recovery, read the
  [execution-machine boundary](../../../docs/architecture.md#execution-machine-boundary-direction-local-execution-today).
- **Discovery/queue:** read the giving-reviews requirements and R1–R3/R8–R10 in
  [PRODUCT.md](../../../PRODUCT.md) for cross-repository personal requests,
  review-cycle identity, serial preparation, cancellation and unavailable CI.
- **Reviewer adapter:** read
  [ADR 0002](../../../docs/adr/0002-restricted-review-preparation.md).
  Demonstrate restrictions at the adapter/tool boundary with each native tool's
  existing authentication; prompts alone cannot establish the contract.
- **Local drafts:** read R4/R7 in [PRODUCT.md](../../../PRODUCT.md) for comment
  choices, regeneration and retained human edits. Use `$rat-persistence` for
  durable edits. A remote pending review is a GitHub write, not a local save.
- **Comparison/revalidation:** read
  [ADR 0006](../../../docs/adr/0006-identify-both-sides-of-review-comparisons.md)
  and R6/R7. Exercise base-only changes, head changes and invalid comment anchors;
  retain examined and human-validated comparison identities separately.
- **Submission:** read the final-preview contract and R5/R11 in
  [PRODUCT.md](../../../PRODUCT.md). Publication requires a subsequent explicit
  user action on that exact preview, with fresh comparison/eligibility evidence.
  Preserve uncertainty after a lost reply and reconcile before duplicate retry.
- **Own PRs:** read C8/C9 in [PRODUCT.md](../../../PRODUCT.md) for selected
  feedback, explicit merge/auto-merge, cancellation and retained task/history.

For CLI/API work, read [transport guidance](references/cli.md) for repository
targeting, pagination, payloads and submission-race limits.
