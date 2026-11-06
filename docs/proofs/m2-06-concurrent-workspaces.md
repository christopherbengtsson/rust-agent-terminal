# Milestone 2 ticket 06: concurrent workspace ownership and capture

Scoped ticket acceptance on macOS, 2026-11-06. Runtime/test source:
`ba17869a6c22701d99ef7516db460bb1b7723875` on
`feature/m2-ticket06-runtime`, `/private/tmp/rat-ticket06-runtime`.
This includes integration `b74faa31` (tickets 03, 05 and 07), capture/restore
checkpoint `e2a1686`, and the final automatic-resize correction. Stable
Rust/Cargo 1.99.1; Darwin 25.2.0 arm64 on the maintainer's MacBook Pro.

## Implemented ownership

The supervisor advances a collection of concrete AgentSession values; their
private launch, execution, cleanup and capture phases remain inside that owner.
No lifetime session thread or generic scheduler was added. Admission compares
canonical worktree Git/common directories, including restored uncertainty and
alias adoption. Separate worktrees can execute concurrently. Selecting an owner
is distinct from starting another conversation.

The dashboard freezes the owner UUID and expected managed session UUID in
`Request::Session`. Input, paste, native keys, resize, Stop and reconciliation
cannot be redirected by a subsequent selection or a replacement conversation.
Observations carry the selected owner identity and bounded session summaries;
App rejects a late snapshot from the previously selected owner. First-prompt
submission retains the identity captured when its modal opened. Legacy request
variants retain their selected-owner adapter. A separate session protocol check
refuses an incompatible live supervisor before using these capabilities.

Selection is supervisor-owned and explicitly persisted. Background metadata for
A cannot select A over B. Restoration retains the latest session per canonical
workspace plus every unresolved execution, failing closed beyond 1024 retained
session records. Historical owners allocate no capture queues or native hook
sockets. Restore never starts coding. Each live owner has separate short private
`/tmp/rat-<uid>-<nonce>` gate/status/settings endpoints, within macOS's 104-byte
Unix socket pathname limit; owner drop removes its endpoint directory.

The sole writer admits at most 32 live capture endpoints. Each has 64 queued
blocks of at most 4096 bytes, one worker block and one blocked producer block:
270336 accepted bytes before the reader's separate finite 1 MiB Stop tail.
Execution admission counts unfinished capture/seal obligations as well as active
or uncertain processes. Each stream has its own transcript handle, counters,
errors and commit ring. One append attempt per stream/pass rotates fairly;
switching streams never forces a commit. Final flush snapshots its own accepted
prefix after EOF, defers until that stream is written, and commits only that
stream. Successful sealing releases its endpoint/handle. Control commands retain
the independent 16-slot queue.

Returned write/partial-write errors can leave A faulted while B progresses.
An actual blocking write/fsync still blocks the sole writer for all streams;
this change does not claim syscall isolation or a measured latency guarantee.
Accepted, written and durable remain separate. Stop during storage trouble keeps
possible-loss context for unread native/kernel output while retaining accepted
bytes. Ordinary quit waits for every owner; detach/client loss leave them alive.

## Public-seam evidence

The approved temporary-Git / dashboard / supervisor / scripted-PTY seam was
used. These are fixtures, not fresh native-tool compatibility claims.

`concurrent_workspaces_keep_controls_capture_faults_and_final_flush_scoped`
passed after the 07 merge (2.04 s). It creates two linked worktrees and two actual
PTY children. It verifies distinct owner/session IDs, input and geometry for A
while B is selected, alias re-adoption preserving A and refusing a duplicate,
and rejection of an old conversation UUID. The actionable rejection then
survives a valid automatic Resize unchanged. With A-only ENOSPC injected, B's
independent durable prefix increases. Stopping A leaves B running and producing;
repair/retry commits A's final accepted prefix while B continues. A's history
contains its own marker and no B marker. A subsequent explicit A conversation
and the still-running B both terminate on ordinary quit; both observed native
fixture PIDs are confirmed absent.

`keyboard_switches_live_workspaces_and_reconnects_in_normal_and_small_ptys`
passed at the final App source (4.18 s). Actual native-pane keys and `w`, `k/j`,
Enter switch between two surviving fixture sessions at 160×40 and 60×18. Input
sent after returning to A reaches A. Abrupt dashboard loss/reconnect retains both
original session UUIDs and B's explicit selected-owner identity; neither tool is
launched again. Shutdown accounts for both. Its harness retries a refused new
controller until the prior socket EOF is observed, preserving the existing
single-controller policy.

Final parsed-cell captures:

- [A, 160×40 cells](screens/m2-06-A-160.json) and
  [render](screens/m2-06-A-160.png).
- [B, 60×18 cells](screens/m2-06-B-60.json) and
  [render](screens/m2-06-B-60.png).

The normal view preserves reviews above the project/worktree tree and the large
native session pane, matching the vision's composition. Compact mode hides the
sidebar and preserves native interaction plus visible return/interrupt controls.
Long identity/path text truncates in compact mode. These are rasterizations of
actual outer-PTY parsed cells with the repository's Swift renderer; they do not
establish Terminal.app/iTerm compatibility.

Visual inspection caught a real intermediate defect: Select B queued an
automatic resize using B's owner target with A's still-visible conversation ID.
The safety check rejected it, but the rejection polluted B's status. The public
App viewport regression failed before the correction and passed after it:
resize now waits for the selected owner's observation. The final PTY test also
asserts the spurious rejection is absent.

The unchanged
`stopped_history_survives_restart_and_missing_checkout_without_launching`
passed after the merge (2.18 s). Resume's missing-checkout refusal stays visible
through background geometry; history remains readable without coding restart.
An intermediate adoption transition defect found by this test was corrected:
App accepts the new owner after the old owner's pending-adoption snapshot.

Additional focused ownership regressions passed after the merge:

| Boundary | Test | Result |
| --- | --- | --- |
| Session-local short Codex hook path; malformed evidence stays Unknown | `codex_status_requires_current_attributed_evidence_despite_liveness_and_prose` | pass, 3.91 s |
| Lost launch acknowledgement and same-request recovery | `lost_launch_acknowledgement_recovers_original_request_and_child` | pass, 1.37 s |
| Stop during stalled preparation; no late launch | `storage_stalled_prepare_keeps_stop_control_and_never_launches_after_cancel` | pass, 1.64 s |
| Wrong final completion cannot acknowledge quit | `mismatched_final_capture_completion_cannot_acknowledge_shutdown` | pass, 1.26 s |
| Partial ENOSPC retains exact bytes; durable retry | `storage_partial_enospc_retains_exact_capture_and_retry_removes_error_after_durability` | pass, 1.72 s |
| Explicit cleanup remains selected and safe | `explicit_cleanup_warns_previews_cancels_and_deletes_only_selected_history` | pass, 2.28 s |
| Failed capture install retains the correct EOF obligation | `failed_capture_installation_never_borrows_previous_sessions_eof_proof` | pass, 1.99 s |
| Outstanding creation remains gated after Stop | `stop_during_outstanding_creation_never_releases_late_gated_process` | pass, 1.05 s |

Seven sole-writer tests pass for independent prefixes/final flush, batched dirty
handles across switches, sparse/control progress under actual queue saturation,
A-only partial-write recovery while B seals, restored durable prefixes,
unselected uncertain reservations/selection stability, and finite 32-stream
admission with seal release. App (21) and UI (12) filtered tests pass, including
separate Running/approval-needed versus Running/Unknown workspace statuses in
normal and compact rendering. The final selected-owner viewport regression was
rerun red then green after that filter run.

## Checks and limits

Final locked build, formatting and warnings-denied all-target Clippy pass.
The full `cargo test --locked --all-targets` aggregate is pending the fresh
integration merger after tickets 06 and 08, as directed by the parent task.
Focused acceptance above is complete; no aggregate pass is claimed.

Fresh native Claude/Codex concurrent-workspace interaction, six-session workload
measurements, actual reboot and complete milestone acceptance are not established
here. Tickets 07/08 retain their own exact source/version native qualification;
changed endpoint/collection paths require final ticket 19 revalidation. This
record does not relabel their evidence or the completed Milestone 1 proof.
