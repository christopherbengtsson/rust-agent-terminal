# Milestone 2 prerequisite integration

2026-11-06; native macOS, stable locked Rust toolchain. Integration branch
`integration/milestone-2-coding-coordination`. These are scripted dashboard,
supervisor, PTY, real temporary Git and private storage checks, not new Claude
Code/Codex native-version qualification.

Ticket 01 merged at `2b2a255`, ticket 02 at `55a8563` with evidence update
`6a5663d`, and ticket 04 at `8c9f7f0`. The only final integration conflict
appended independent ownership tests; both sets were retained. Ticket 02's earlier
merge against ticket 01 preserved exact durable-operation attribution and placed
authorization after owned input/capture installation.

The first serial union validation at `8c9f7f0` passed locked build, formatting
and warnings-denied all-target Clippy. The all-target command passed 32 unit
checks with one intentional subprocess probe ignored, then 62 ownership checks
with two opt-in measurements ignored. Its one failure was
`storage_saturation_keeps_navigation_retry_stop_and_exact_retained_bytes` at
`tests/ownership.rs:3227`: strict receipt-ledger JSON parsing returned EOF
(line 1, column 47). Ownership elapsed 435.56 seconds. The command stopped before
scripted-session/screen targets; this is not an all-target pass.

Every other union ownership scenario passed, including sustained capture,
partial-spawn failure matrix, cleanup failure/retry, outstanding creation Stop,
session-specific capture EOF after restart, mismatched/duplicate durable receipts,
failed launch-result retry, lost acknowledgements, launch crash windows, stalled
storage cancellation, original native outcomes, descendant Stop/quit and legacy
live-owner refusal.

The independently prepared fixture-only correction `ede8d43` merged with
`--no-ff` at `265ca2e`. It formats a complete bounded JSON receipt before writing
the file; strict parsing and exact retained-prefix comparisons remain unchanged.
Its focused saturation regression passed in 4.10 seconds. Locked build, formatting
and warnings-denied all-target Clippy passed again. A complete post-correction
all-target rerun is active; final aggregate results are pending. No crash-atomic
ledger guarantee is inferred from this correction.

Tickets 01, 02 and 04 are resolved against their demonstrated scoped acceptance
behavior and the focused fixture correction; final aggregate gate completion is
tracked separately here. This opens dependent implementation without pretending
the pending rerun has passed.

For ticket 04, actual linked/detached/locked/prunable/bare/missing/alias repositories,
saturated dispatch and cancellation passed. The native filesystem rejects an
actual invalid-byte directory creation attempt with EILSEQ. Adoption safely rejects
that unavailable path; parser and wire tests preserve its original bytes exactly.
A successful invalid-byte checkout cannot be created on this tested filesystem;
no such successful-checkout claim is made. Existing native CLI proofs retain their
original source/version/workload scope.
