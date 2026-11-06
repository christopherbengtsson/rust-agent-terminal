# Issue tracker: Local Markdown

Specs and tickets for this repo live in `.scratch/<feature-slug>/`.

## Conventions

- Spec: `spec.md` within the feature directory.
- Tickets: `issues/<NN>-<slug>.md`, one file per ticket, numbered from
  `01` in dependency order.
- Triage state: a `Status:` line near the top, using the strings in
  `triage-labels.md`.
- Dependencies: a `Blocked by: 01, 02` line referencing tickets within
  the same feature; use `Blocked by: None` when independent.
- Completion: a `Resolved: yes` line records completion separately from
  triage state. A ticket is unblocked when every blocker is resolved.
- Comments: append under `## Comments`.

Keep `.scratch/` available for Git tracking so specs and tickets travel
with the repository.

## Publishing and fetching

When a skill says "publish to the issue tracker", write the spec or
individual ticket files at the paths above.

When a skill says "fetch the relevant ticket", read the referenced file.
Resolve bare numbers within the specified feature; ask for the feature
if the number is ambiguous.

## Wayfinding operations

- Map: `.scratch/<effort>/map.md`, containing Notes, Decisions-so-far,
  and Fog.
- Child: `issues/<NN>-<slug>.md`, with a `Type:` line containing
  `research`, `prototype`, `grilling`, or `task`.
- Lifecycle: wayfinding children use `Status: open`, `claimed`, or
  `resolved`; this is distinct from ordinary ticket triage.
- Blocking: use `Blocked by:` with child numbers. Every blocker must
  have `Status: resolved` before work begins.
- Frontier: select the first open, unblocked child by number.
- Claim: set `Status: claimed` and save before starting work.
- Resolve: append the answer under `## Answer`, set `Status: resolved`,
  and append a summary and link to the map's Decisions-so-far.
