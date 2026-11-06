# Milestone 2 ticket 03: concrete Agent session ownership

2026-11-06, native macOS, stable locked Rust toolchain. The established seam
remains a real dashboard, supervisor and scripted PTY child in isolated private
state and an actual temporary Git checkout. These checks do not establish a new
Claude Code/Codex version qualification.

## Ownership and interface

`session::agent_session::AgentSession` is a concrete owned Rust value polled by
the supervisor. Private Launch, Execution and History records keep the durable
prepare/authorize/result progression, actual child and PTY ownership, provisional
native controls, original native exit outcome, bounded input/capture, Stop retry,
EOF, final-flush and confirmed-seal obligations together. Matching operation
completion remains the only way to settle an attributed durable obligation.
Cancellation cannot be undone by a late creation result or authorization commit.

The supervisor passes explicit session requests and storage completions, advances
the owner, and projects a cloned Observation. It cannot mutate session identity
or reconstruct private phase flags. A prepared continuation is consumed by the
owner's advance pass, even without a connected dashboard. The supervisor retains
canonical Git admission, context selection, Git workers/cancellation, sole-owner
election, socket framing, bounded/coalesced delivery and controller ownership.
Aggregate quit combines owner readiness with completed Git work. SQLite and the
single storage worker remain in persistence. No wire/schema, new provider seam,
per-session event loop or lifetime thread is introduced.

Existing bounded helpers and memory budgets remain: one creation worker, native
input queue of 32 blocks capped at 4096 bytes, one receipt worker, sole PTY reader,
1 MiB / 256-block Stop tail, and one Stop worker at a time. Restored process
identity remains evidence for explicit reconciliation, never signal authority or
a request to start/resume coding.

## Verification

This preserves behavior behind the already approved dashboard/supervisor/fixture
seam. It is an ownership refactor, so the existing regression suite supplies the
behavioral baseline; no intentionally failing product behavior is invented.
The existing mismatched-result-completion regression is strengthened to verify
actual input, terminal resize and rendered response on the same execution while
the genuine result completion is held. It then confirms Stop on that same session
while acknowledgement is still withheld, and releases the exact result receipt
only afterward. A successful later result acknowledgement cannot revive stopped
execution. Existing receipt-gap, partial-spawn, stale receipt, final capture,
storage fault and reconciliation cases remain unchanged.

Implementation `125bf5a`, based on prerequisite integration `bb306b3`, passed
locked build, formatting, warnings-denied all-target Clippy and `git diff --check`.
The bin state/render/Git suite passed 32 checks with one subprocess probe ignored
directly; its real terminal-restoration driver passed. After the coordinator
released the serial PTY slot, all 13 focused ownership cases passed:

- `mismatched_` ran all three operation-attribution cases, including strengthened
  provisional native controls and Stop, in 2.93 seconds.
- `partial_spawn_` ran the five setup-window matrix and actual failed-cleanup
  repair cases in 7.50 seconds.
- Outstanding creation cancellation passed in 1.04 seconds; failed capture
  installation after a prior session passed in 1.91 seconds.
- Held native exec receipt passed in 0.98 seconds; stalled Prepare cancellation
  passed in 1.82 seconds; stalled Authorize/result controls passed in 2.79 seconds.
- Failed final commit/explicit retry passed in 2.36 seconds; failed launch-result
  write plus lifecycle save/explicit retry passed in 1.87 seconds.
- Stop, ordinary quit and natural native exit with a separate-process-group
  foreground tool passed in 8.01 seconds, preserving an unrelated sentinel.

Each used `cargo test --locked --test ownership <filter> -- --test-threads=1`.
No test assertions were weakened to accommodate extraction. The integration tip
`50c0bae` was then merged; it changes only prerequisite evidence documentation.
Post-merge locked build, formatting and warnings-denied all-target Clippy passed.

The full combined 03/05 all-target gate belongs to the integration coordinator
and is pending. The prerequisite aggregate record preserves two independently
reproduced byte-boundary fixture failures; their diagnosis belongs to a separate
change, and these focused passes do not relabel that earlier aggregate as green.
No sustained-resource or styled-screen rerun is claimed for this owner change.

## Limits

No new real Claude Code/Codex interaction, physical reboot, power-loss or full
multi-session workload measurement is claimed. Existing versioned native proof
records retain their original source/workload scope. This ticket does not claim
C1–C9 or Milestone 2 release acceptance.
