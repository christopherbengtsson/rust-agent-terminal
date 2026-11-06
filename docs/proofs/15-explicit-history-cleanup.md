# Ticket 15: explicit retained-history cleanup

2026-11-06, macOS 26.2 (25C56), arm64, stable rustc/cargo 1.99.1, debug build.
Implementation verification revision: `7f8a41d`. It includes integration
`a5b406f` and ticket 17's reusable telemetry checkpoint `2c9c54c` (merge
`1d86070`). The approved seams are real dashboard/supervisor/scripted PTYs in
real temporary Git repositories, plus existing App/event and Ratatui rendering
boundaries. Outer PTYs use 160×40; render checks also use 70×20. Operations use
isolated private `/private/tmp` state and real SQLite/chunk files. This adds no
real Claude/Codex compatibility claim, physical disk-full or sudden power-loss
claim, or new resource/latency measurement. Ticket 18 retains final union checks.

## Implemented policy and durable boundary

The sole persistence worker samples the state filesystem's user-available space
using `statvfs`, once per second between worker operations. It warns strictly
below **1,073,741,824 bytes (1 GiB)**; equality and higher values do not warn.
The multiplication uses a u128 intermediate and saturates at u64::MAX. Failed
samples report unavailable capacity. A blocked filesystem call can delay the
next sample; the separate storage-wait/capture indicators remain available.
No filesystem/stat/SQLite operation runs in App/rendering or supervisor control.

`g` opens a bounded eight-entry page. `j`/`k` selects, `n`/`p` pages, Enter
previews the selected UUID/effect, Shift-D confirms only that reviewed UUID,
and Esc cancels/back. Native focus retains native shortcuts. Rows show actual
task/session identity, age and physical allocated transcript bytes (`st_blocks *
512`), including uncommitted tails. Oldest known eligible stopped sessions come
first; insertion order breaks timestamp ties. Unknown legacy ages remain unknown.
Directory listing/unlink work advances at most 64 entries per worker pass.
Unsafe/unreadable sizes are unavailable rather than zero. These are transcript
allocation estimates: shared SQLite pages, filesystem snapshots/compression and
unrelated disk consumers mean successful cleanup need not free sufficient space.

Schema 3 migrates creation timestamps as NULL for existing records and adds a
final-capture seal and retained/deleting/deleted state. A current runtime is
sealed only after accepted PTY EOF, drained capture, successful final flush,
committed metadata and confirmed stopped execution. The worker verifies its
saved lifecycle/execution and closes the retained transcript File before the
seal commits. Stopped alone is insufficient. Startup cannot manufacture a seal
from a stopped lifecycle; legacy unsealed and interrupted/uncertain rows remain
visible but unavailable. `y` retries failed final storage/sealing after repair.

The worker and supervisor recheck selected eligibility. A FULL SQLite transaction
commits `deleting` **before the first unlink**. Deleting/deleted segments are
unavailable to readable-history navigation and append. Only canonical managed
UUID/ordinal chunk names are considered. An O_DIRECTORY/O_NOFOLLOW handle anchors
`fstatat(AT_SYMLINK_NOFOLLOW)` and `unlinkat` to the private state directory inode;
symlinks, non-regular files and hard-linked chunks are refused. No saved checkout
or arbitrary record path can authorize deletion. Unlink is incremental; directory
`sync_all` precedes the FULL completed-marker transaction. Completion publishes
updated current-segment counters and freshly sampled capacity before its event.
History cache/position are reset. Other segments' files/counters are preserved.

Failed intent leaves every chunk untouched. Later failure retains an incomplete
intent, never a completed outcome. Explicit selected retry or restart finishes
surviving chunks only for a previously committed explicit intent. Session/task,
native conversation IDs and launch dedup outcomes remain; later metadata Save
cannot resurrect deleted transcripts. Checkout files/native tool records are
never deletion inputs. No automatic age/pressure/exit/archival retention policy,
VACUUM or launch-result pruning was introduced. Cleanup does not clear accepted
pending bytes, write faults or possible-loss context. Only actual durable capture
success clears the capture fault; `y` retry and `s` Stop remain available.

## Observed checks and red/green findings

The initial real-dashboard scenario failed for absent Low storage UI. The first
working UI exposed that launch auto-attaches and uppercase D carries Shift;
the harness now returns dashboard focus explicitly and App accepts Shift-D only
in cleanup context. A later failure-boundary run caught a real accounting race:
Deleted could arrive before progress reflected the completed durable extent.
Publishing accounting/capacity before completion fixed it. Cancellation is tested
through App effects and unchanged actual transcript bytes; preview copies remain
bound to their UUID even when a later observation reorders the list.

Six focused cleanup integration scenarios passed together in 7.31 seconds:

- Warning, oldest suggested stopped segment, explicit selection/preview/cancel/
  confirm, unchanged checkout sentinel, continued low capacity and clean quit.
- Active/current unflushed refusal; failed intent unlinks nothing; a multi-chunk
  segment loses exactly one chunk before its owned supervisor is killed at a
  controlled post-unlink stall. Restart finishes that persisted intent, hides
  deleted history and retains another segment byte-for-byte, its accounting,
  launch outcome and checkout working change.
- Unlink, directory-sync and completed-marker failures remain visibly incomplete
  and unavailable to history reading. Missing files are not treated as completed
  cleanup; explicit repair/retry reaches the completed marker and zero current
  durable extent only after success.
- Stopped capture with accepted pending bytes remains ineligible. Deleting an
  older sealed segment leaves the current ENOSPC capture fault/pending bytes and
  low-capacity warning intact. Repair plus RetryStorage actually commits those
  retained bytes before clearing the fault; capacity remains low.
- Real version-one migration exposes unknown ages, eight plus two bounded rows,
  and refuses arbitrary file paths without touching checkout contents. Legacy
  Stopped without final-capture proof remains unavailable.
- A selected canonical chunk replaced with a symlink to a checkout sentinel has
  unavailable size and is not eligible. Direct delete rechecks and refuses the
  unsafe object, preserving both symlink and target. Explicit repair/retry succeeds.

The extended existing native-style Codex recovery scenario passed in 3.09 seconds:
original and resumed managed segments share the same native conversation UUID;
selected deletion removes only the original transcript, keeps resumed bytes and
native identity, and normalizes readable-history index zero to the resumed segment.
This is a scripted tool-wrapper assertion, not a new native CLI experiment.
Three focused App/render checks passed, including unknown age, physical size,
incomplete deletion, preview/cancel/confirm UUID binding, native-focus shortcuts,
normal/small controls, exact threshold boundaries, u64::MAX and unavailable capacity.

Reproduction:

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --test ownership cleanup_ -- --nocapture
cargo test --locked --test ownership codex_native_identity_survives_exit -- --nocapture
cargo test --locked --bin agent-terminal cleanup_ -- --nocapture
cargo test --locked --bin agent-terminal storage_warning -- --nocapture
cargo test --locked --all-targets -- --skip sustained_capture_crosses_caps_while_attached_focused_detached_and_lagged
```

Build, formatting and warnings-denied all-target Clippy passed. At `7f8a41d`,
the final filtered all-target regression passed: 26 unit tests (one intentionally
ignored subprocess probe was exercised by the passing terminal-restoration
driver), 46 ownership scenarios passed in 94.10 seconds (one ignored measurement,
one filtered sustained workload), scripted interaction passed in 26.99 seconds,
and screen integration passed in 8.49 seconds. This includes all six cleanup
scenarios and the extended original/resumed native-style identity scenario in
the combined run. The unchanged
long sustained resource scenario is deliberately excluded here; ticket 17's
ignored multi-minute measurement is likewise not represented as executed by this
regression. Ticket 18 owns the final combined gate. Faults are debug-only files
in each isolated state directory (`proof-storage-available`, delete intent/unlink/
post-unlink/directory-sync/complete and ticket 14 write boundaries); production
ignores them. They exercise real writes/unlinks/SQLite commits but do not fill a
user volume or prove physical drive durability.


The cleanup screen temporarily uses the existing native-session pane while in
dashboard focus, preserving reviews/tree/status composition. Normal and small
Ratatui render checks preserve explicit preview, selected UUID and cancel/delete/
retry/stop controls. No graphical terminal screenshots or presentation timing
measurement were added in this slice; ticket 17/18 retain final UI measurement
and visual evidence. No final healthy resource sample is claimed here.

A final read-only process audit found one old harness/supervisor pair from the
initial unbounded quit-key experiment, separate from the passing final suite.
The exact isolated `/private/tmp/.tmp1KHBCz/state/supervisor.sock` authenticated
peer PID 29935 and acknowledged Shutdown with `stopped=true`; only its directly
identified old harness PID 29932 was then terminated. The corrected harness uses
separate Escape/quit input and a bounded exit deadline. No broad process search
or normal user endpoint was used for termination.
