# 10: Manually prepare a native review draft

Status: ready-for-agent
Resolved: no
Blocked by: 02, 03, 08, 09

**What to build:** Explicitly ask either qualified native reviewer to prepare one PR, then open its validated structured suggestions beside the matching comparison as a retained local draft.

**Acceptance coverage:** R3; R10 preparation completion/failure distinctions.

## Acceptance criteria

- [ ] An explicit manual action identifies the PR and selected reviewer, explains usage, resolves pinned context/guidance and starts the qualified Claude or Codex inspection path independently of coding-worktree admission.
- [ ] Enforce the proven restrictions during real execution. No arbitrary commands, automatic review builds/lints/tests, coding-checkout mutations or GitHub writes occur.
- [ ] Validate structured findings, location/context and completion evidence. Successful findings start undecided; complete no-findings output is distinct from malformed, partial, interrupted and failed output.
- [ ] Retain the examined comparison, guidance provenance, tool identity, attempt identity and exact generated content through attributed durable saves before advertising a ready draft.
- [ ] Preserve existing manual text and prior attempt results; a retry cannot silently overwrite human edits or reuse selection from generated suggestions.
- [ ] Keep one manual preparation active initially, expose progress and an explicit stop/cancel path, and maintain responsive coding navigation. Serial queued scheduling is expanded in ticket 11.
- [ ] Verify full workflow behavior with controlled reviewers and both real qualified native integrations, including missing CI, absent originating spec, malformed output and interruption. Follow agent cost controls and record native versions.

## Scope and handoff

Any provider incompatibility discovered after the early proofs returns as an explicit decision. Do not ship a fixture-only success path as native support.

## Comments
