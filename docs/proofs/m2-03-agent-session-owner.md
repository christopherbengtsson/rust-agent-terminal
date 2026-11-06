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

Pending final integrated validation. Locked build and warnings-denied all-target
Clippy pass; bin state/render/Git checks passed 32 with one subprocess probe
ignored directly. Final ownership/all-target results will be recorded after the
coordinator releases the serial PTY test slot.

## Limits

No new real Claude Code/Codex interaction, physical reboot, power-loss or full
multi-session workload measurement is claimed. Existing versioned native proof
records retain their original source/workload scope. This ticket does not claim
C1–C9 or Milestone 2 release acceptance.
