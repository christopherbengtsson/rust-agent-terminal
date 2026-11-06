# Restrict the background reviewer to revision-pinned inspection

Accepted during the architecture interview on 2026-11-06. Background review
preparation receives a revision-pinned repository snapshot and reported CI
evidence, with enforced restrictions preventing arbitrary shell execution,
task-checkout mutation, and GitHub writes. The product obtains repository
context and validates the resulting draft; explicit human submission remains a
separate operation.

This trades the reviewer's general coding capabilities for an enforceable
preparation contract: it cannot automatically run builds or tests, publish a
review, or merge. Prompt instructions alone do not establish the restriction.
Both native integrations must demonstrate that the restriction can coexist with
their existing authentication; feasibility is not yet established, and any
incompatibility returns as an explicit decision rather than a silent fallback.
