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

## Completed post-correction checks

The second serial all-target command (product source `265ca2e`, later
documentation-only ticket resolution `bb306b3`) passed 32 unit checks with one
intentional ignored probe. Ownership completed in 455.38 seconds: 62 passed,
one failed, two opt-in measurements ignored. Strict ledger saturation and the
sustained resource scenario both passed. The sole failure was now
`healthy_output_reports_real_durable_boundaries_within_one_second` at
`tests/ownership.rs:1360`: final snapshot durable bytes 7790 versus accepted
output bytes 8182. All 15 observed flush samples met the one-second target
(accepted ages 162–296 ms; successive completion intervals 268–278 ms).
The all-target command again stopped before the two remaining integration targets.

An unchanged isolated healthy-output rerun failed in 5.19 seconds at the same
final equality: 7902 durable versus 8182 output bytes. All measured timing samples
also met the target (accepted ages 167–292 ms). This is a reproducible final-prefix
assertion failure requiring diagnosis; no product fix, weakened assertion or
claim of harmless host timing is made here.

The remaining `scripted_session` target passed in 30.31 seconds. An orchestration
mistake briefly overlapped it with `session_screen`; that screen target failed
in 7.76 seconds at `tests/session_screen.rs:305`, parsed bytes 2660 versus emitted
2809. The unchanged screen target was then rerun alone and failed again in 7.79
seconds, parsed 2646 versus emitted 2809. Thus overlap does not explain away the
failure. This second reproducible final-byte equality concern also needs
diagnosis. No capture assertions were changed.

Build, formatting and warnings-denied all-target Clippy remain green for this
union, but there is no clean complete all-target pass. Scoped ticket acceptance
is retained as recorded above; the integration gate remains failing on these
two explicit concerns. A fresh owner will diagnose the failures before further
integration claims. Completed ticket 01/02/04 and fixture-fix worktrees were
removed only after clean status and ancestry confirmation; their branches and
unrelated worktrees were retained.


## Finite-output boundary diagnosis and correction

The fresh isolated implementer reproduced both unchanged failures from `50c0bae`
serially through the existing dashboard/supervisor/scripted-child seam:

```sh
cargo test --locked --test ownership healthy_output_reports_real_durable_boundaries_within_one_second -- --exact --nocapture --test-threads=1
cargo test --locked --test session_screen -- --nocapture --test-threads=1
```

Healthy-output red: 7,958 durable versus 8,196 accepted bytes after the four-second
sample window; 15 samples, accepted ages 177–290 ms, 5.21 seconds total. Screen red:
2,632 versus the exact expected 2,809-byte delta after 1,300 ms; 7.57 seconds total.
These are the same final-boundary symptoms as the earlier union failures.

The ranked hypotheses were incomplete finite producer, completed output awaiting
its healthy flush, and lost/duplicated capture. The smallest discriminating signal
is the known complete stream byte count, rather than a snapshot at elapsed nominal
sleep time. Each stream writes 200 fourteen-byte records plus nine prompt bytes;
its 200 real five-millisecond sleeps are minimum waits. The final numbered record
also precedes the last sleep and trailing prompt, so that record alone does not
establish completion. Three streams add exactly 8,427 bytes to the ready fixture
baseline. The original observed 8,196 accepted total is itself below the complete
8,527-byte total (100-byte ready baseline), establishing an incomplete producer
at the old assertion boundary without inferring that from timing alone.

The test-only correction `6ee8a7b` observes that complete finite output before
asserting final durability. Producer completion is bounded to ten seconds, followed
by a separate strictly sub-one-second durable catch-up check. The original sampled
one-second age/interval assertions and exact final byte equality remain; the test
now additionally checks every retained ring commit's age and interval below one
second, verifies contiguous sequences, and requires the full known output count.
No sample or commit is filtered. Missing bytes time out and excess bytes fail.
The screen test retains a real 1,300-ms interval with no connected dashboard or
controller and then polls to the exact 2,809-byte boundary with a five-second bound;
identity, screen marker and exact delta assertions remain. This preserves the
actual disconnected capture exercise while removing elapsed time as a completion
claim. Product source and fixture output were unchanged.

Pre-merge green observations: healthy output completed at 4.211 and 4.170 seconds,
then became fully durable after 237 and 293 ms respectively. Both runs retained
all 17 commits and passed the stricter per-commit timing assertions (5.50 and 5.52
seconds total). Screen output completed exactly 2,809 bytes at 1.404 seconds,
then the complete resize/style/reconnect test passed in 10.17 seconds. The previous
1,300-ms observation cut through real ongoing output. An exploratory immediate
controller reconnection hit the existing connection-release race (BrokenPipe);
retaining the original disconnected interval removed that unrelated probe.
Formatting and warnings-denied all-target Clippy passed for the correction.

The branch then merged integration `455c4c0` (ticket 03's session-owner refactor)
cleanly at `133d87b`. The focused green observations above apply to `6ee8a7b`
on the pre-extraction `50c0bae` product source. The integration owner will run the
combined required gate after merging this correction; no post-extraction focused
rerun, full all-target pass or new native CLI compatibility is claimed here.

Post-merge `133d87b` locked build, formatting check, warnings-denied all-target
Clippy and `git diff --check` passed. PTY validation remained serial; the slot was
released to ticket 05's dashboard proof before final combined validation.
