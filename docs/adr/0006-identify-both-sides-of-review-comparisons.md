# Preserve both sides of the reviewed comparison

Accepted during the architecture interview on 2026-11-06. Each review draft
records the PR head revision and the base/comparison revision used to produce
its diff, retaining the original context alongside later human validation.
Before submission, refresh the comparison and require revalidation if it has
changed, including when the target branch moved without new author commits.

This costs more revision tracking than identifying a draft by PR head alone,
but makes the relationship between examined code, displayed diff, validated
comments, and publication explainable. The integration must establish the exact
comparison semantics and test base-only changes as well as head changes.
