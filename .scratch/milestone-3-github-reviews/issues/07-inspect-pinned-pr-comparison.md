# 07: Inspect a pinned PR comparison

Status: ready-for-agent
Resolved: no
Blocked by: 06

**What to build:** Open an inbox PR with its description, revision-specific diff, relevant repository context and reported CI evidence, independently of any coding checkout.

**Acceptance coverage:** R3 context and CI; R6 comparison foundation.

## Acceptance criteria

- [ ] Capture the proven head, target/base and comparison identity explicitly, retaining enough context to explain what code was examined. A target-branch change with an unchanged head must not be hidden.
- [ ] Display a navigable changed-file list and pinned file/diff content with stable context, path handling and explicit unavailable/binary/omitted content where applicable.
- [ ] Obtain repository context for unregistered repositories without adopting a workspace, running project setup or modifying an existing coding checkout.
- [ ] Show description and CI evidence with missing, pending, failed, passing and unavailable states distinguished; never infer passing from missing evidence.
- [ ] Keep Git, filesystem and network work in bounded cancellable workers. Reject stale completions that would retarget the current PR or comparison.
- [ ] Keep pinned comparison production distinct from Working and Whole workspace capture. Extract presentation only if actual shared behavior earns depth; a two-field forwarding module is not a prerequisite.
- [ ] Verify supervisor observations and rendered content using real temporary Git repositories plus controlled GitHub responses for head/base changes, renames, local working changes, cancellation and unavailable context.

## Scope and handoff

This is inspection only, with no reviewer launch or publication. Do not reuse truncated coding inspection artifacts as complete reviewer input.

## Comments
