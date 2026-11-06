# Milestone 2 ticket 01: durable session operation attribution

2026-11-06; macOS 26.2 (25C56), stable rustc 1.99.1. The agreed seam
remains real dashboard/supervisor/scripted PTY processes, real temporary Git
repositories, isolated private storage and the existing sole SQLite writer.
This is fixture evidence, not a new Claude Code/Codex version qualification.

## Implemented boundary

Preparation, authorization, launch-result recording, other session metadata,
final capture flush and history sealing carry operation UUID, session UUID when
available, request identity and purpose through commands and successful/failed
completions. Only a command accepted by the bounded writer queue creates an
outstanding obligation. Completing it requires equality of every field; duplicates
and altered receipts do not decrement another command's obligation. Existing
bounded command/completion queues also bound the outstanding-operation map.

Prepare/Authorize additionally require the current session and request and respect
Stop cancellation. Launch acknowledgement requires the exact launch-result
operation. A failed result remains provisional despite later successful lifecycle
metadata. Explicit storage retry gives the result a new operation identity and
saves the latest execution facts without creating or releasing a child.

Final flush/seal use the original session's immutable request, independently of
a later rejected launch request. A rejected launch result instead carries that
rejected request's identity. Metadata-pending retains its existing wire meaning;
final-capture obligations are accounted separately for shutdown. Seal is confirmed
on matching commit, not enqueue. Successful metadata recovery clears only its own
error text, leaving a newer execution/input/control diagnostic intact.

## Regression evidence

The initial preparation test failed with `metadata_pending=false` while the exact
receipt should have remained held. The final-capture test separately failed when
shutdown was acknowledged before its matching receipt. Each passed after its
attributed completion implementation.

Private debug-only `proof-storage-completion` markers hold a real committed
Prepare/result/final-flush completion, inject altered operation/session/request/
purpose successes and failures on the existing completion channel, then signal
`proof-storage-completion.ready`. Tests wait for that boundary before observing
provisional state; removing the marker releases the genuine receipt. No second
writer or product-facing injection command is introduced. `proof-storage-replay`
replays the previous committed operation and a duplicate failure before the next
real commit, including across two distinct native runtimes. Fixture teardown
releases only its private proof markers.

Final focused checks passed:

- Altered preparation receipts cannot create a child; release creates exactly one.
- Altered result receipts preserve live execution and leave the result provisional.
- Altered final-flush receipts cannot acknowledge shutdown.
- Duplicate/prior-session successes and failures preserve the new execution; replay
  of its launch request returns the same result and process identity.
- A genuine failed result write remains provisional after successful Stop/lifecycle
  metadata; explicit result retry returns its retained result while execution stays
  Stopped, with no replacement process.
- Existing failed native exec/restart, native process outcome/history/resume and
  unavailable-resume scenarios pass unchanged after the final corrections.
- Locked build, formatting and warnings-denied all-target Clippy pass.

Earlier focused runs passed seven `launch_` scenarios, including lost success/failure
acknowledgements, held exec receipt, gate arguments and all four injected launch
crash windows. Eight `storage_` scenarios passed (one opt-in measurement ignored),
including stalled Prepare/Authorize/result cancellation, partial ENOSPC, delayed
writes/sync, abrupt-prefix recovery, saturation/Stop and final-commit retry.

The first aggregate run passed 28 unit checks (one intentional subprocess probe
ignored), then 53 ownership checks, with four failures and two opt-in measurements
ignored. Three failures exposed attribution integration bugs: final-capture
obligations initially changed metadata-pending semantics and hid a durable result;
using mutable latest-request identity also stranded final capture after a rejected
request. Those were corrected and the failed-gate, native-outcome and unavailable-
resume checks passed unchanged in focused reruns. The resource scenario failed its
existing three-second detached `flood13-complete run=3` observation while other
branches also exercised PTYs. Its assertion is unchanged; this run does not claim
that failure was established as host timing rather than a product issue.

A serialized integrated all-target run is still required, including the sustained
resource scenario, scripted-session and screen targets. Ticket completion is
therefore left unresolved for the integration owner. Earlier focused receipts are
not substituted for that final whole-union gate.

## Compatibility and limits

No public Request/Observation representation, SQLite schema, durable launch-result
format or migration changes here. Operation UUIDs are private to the current owner;
a restart cannot deliver an earlier worker's completion. Retained same-request
outcomes, prepared/authorized process evidence and conservative reconciliation
remain the restart contract. No automatic execution or restored-PID signal
authority is introduced.

The existing exclusive supervisor lock is acquired before opening/migrating the
database; a live owner is never displaced or migrated underneath. This change
introduces no incompatible wire message for a live Milestone 1 owner. New commands
or schema changes in later tickets must detect incompatible live owners before
using those capabilities. Unchanged wire compatibility does not implement that
future capability handshake.

Native Claude/Codex interaction on changed code, physical reboot, abrupt power loss
and later multi-session scheduling remain unverified by this ticket. Existing
native-tool proof records retain their original scope.
