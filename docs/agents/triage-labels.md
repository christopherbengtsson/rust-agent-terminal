# Triage labels

Local tickets record the configured role in a `Status:` line.

| Canonical role | Status value | Meaning |
| --- | --- | --- |
| needs-triage | needs-triage | Needs maintainer evaluation |
| needs-info | needs-info | Waiting for more information |
| ready-for-agent | ready-for-agent | Fully specified for agent implementation |
| ready-for-human | ready-for-human | Requires human implementation |
| wontfix | wontfix | Will not be actioned |

When a skill applies or removes a triage label, update the ticket's
`Status:` line using this mapping. Completion is recorded separately
as described in `issue-tracker.md`.
