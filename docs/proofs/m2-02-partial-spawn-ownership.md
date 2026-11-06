# Milestone 2 ticket 02: partial process creation ownership

2026-11-06, Darwin 25.2.0 arm64, stable rustc 1.99.1, debug locked build.
The seam is the agreed real dashboard/supervisor/scripted PTY child, isolated
private state directories and actual temporary Git repositories. These fixtures
do not establish new Claude Code or Codex native compatibility.

One bounded creation worker retains the child, PTY pair, gate listener and nonce
immediately after successful spawn. The supervisor retains that in-flight result
as a reservation and continues serving Stop, Screen and launch queries. Stop
cancels release intent; shutdown cannot acknowledge completion while creation
remains outstanding. A late child returns to the existing installation and
verified lifecycle cleanup path, without authorization or native execution.

Before fallible identity/input/capture installation, the supervisor retains the
actual child and master PTY, plus the gate listener until its handshake worker
owns it. Scope-capture failure keeps the child available for explicit Stop to
retry ownership capture; uncertain cleanup keeps the reservation. Authorization
is queued only after input and capture are installed. Setup failure drops the
release sender and preserves its error despite a later closed-gate receipt.
The existing descendant identity verification, native exit outcome retention,
bounded capture/drain and storage worker remain in place.

The first red external test observed an ordinary running fixture instead of the
requested after-create failure. Its green regression covers five boundaries:
before scope capture, immediately after creation/identity, before input, before
capture and before authorization. Each fails with an actual gated child, runs no
fixture code, confirms cleanup through Stop and process absence, waits for
confirmed final shutdown, then starts a replacement after restart.

The second red external test timed out waiting for creation-worker readiness.
Its green version pauses after actual spawn, verifies same-request retry/query
remain provisional, times a Stop response under one second, refuses replacement
while creation is outstanding, then releases only the worker pause. The returned
child is cleaned without releasing its native gate. Its durable request outcome
identifies cancellation. A separate real failing `ps` executable causes
StopFailed after partial setup: the retained PID and reservation survive,
replacement is refused, and repairing `ps` lets explicit Stop finish cleanup.
Only debug builds inspect the private proof markers; production ignores them.

Native argument/environment/cwd and missing executable behavior use the existing
launch-gate fixtures. Ordinary quit, lost acknowledgements, crash reconciliation,
original native exit results, descendant cleanup, storage trouble and capture
backpressure retain their existing external regressions. Native CLI interaction,
physical reboot and resource/latency qualification beyond these declared fixture
checks remain unverified by this ticket.

A further red/green restart case establishes capture ownership between sessions:
a completed first capture must not make a later failed capture installation
eligible for history deletion. The red run exposed `eligible=true` for the new
session despite no reader observing its EOF. Resetting the per-attempt
`capture_started_here` proof at accepted preparation, and setting it only after
reader installation, made that case pass in 1.94 seconds. The sole storage
worker's existing seal/deletion contract is unchanged; this supervisor-side
change preserves its required session-specific EOF evidence.

## Validation checkpoint

Implementation commits `a877c48`, `3d5b216`; integration merge `6e1f7f8` includes
attributed persistence operations from integration `2b2a255`. Post-merge locked
build, formatting, all-target warnings-denied Clippy and `git diff --check` pass.
The merged four new external tests pass: five-boundary setup matrix and actual
verification-failure/repair (two tests, 6.20s), outstanding creation cancellation
(0.76s), and session-specific capture/EOF proof after restart (1.82s).

The intermediate all-target run passed 28 unit tests (one subprocess probe
ignored directly), then passed 53 ownership scenarios with two failures and two
ignored measurement scenarios. One failure was a real new asynchronous status
boundary: polling the newly configured Codex monitor before child installation
replaced its initial unavailable reason. Polling is now deferred only while
creation/result resources await installation; provisional running execution
still receives semantic polling before result durability. Its isolated regression
passed in 3.53s. The other failed the existing detached flood completion screen
assertion; its unchanged isolated sustained-capture rerun passed in 274.18s.
No cause for that first timing failure is asserted. Gate argv/environment/cwd,
missing executables, lost acknowledgements, launch crash windows, storage stalls,
Stop/quit/descendant cleanup and native outcome fixture checks passed in that
intermediate run. The final serial union all-target suite is owned by the
integration coordinator; this checkpoint does not claim it has completed.
