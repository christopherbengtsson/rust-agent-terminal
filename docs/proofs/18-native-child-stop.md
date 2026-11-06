# Native foreground child ownership correction

The ticket-18 default Codex 0.167.0 `--no-daemon` proof exposed an ordinary
foreground Python tool child in its own process group. Stopping the CLI group
reported Stopped while the reparented child's quiet heartbeat continued. This
is a supported native ownership failure, not a deliberate background escape.
The native failure receipt is maintained by ticket 18 as
`18-stop-gate-failure.json`.

The red external regression was
`cargo test --locked --test ownership stop_terminates_quiet_foreground_tool_in_separate_process_group -- --exact --nocapture`:
in 1.29 seconds it observed the heartbeat growing after confirmed Stopped.
The fixture starts a quiet, bounded Python child with `process_group(0)` and
independent file heartbeat, ignoring HUP/TERM. The final regression additionally
exercises ordinary q, verifies its PID cannot execute, and verifies an unrelated
sentinel remains running. No native-tool compatibility is inferred from it.

The cause is the former lifecycle boundary: it signalled and verified only the
PTY leader's process group. The live owner now captures the gated helper's
identity before authorization, and tracks descendant identities across process
groups on a worker. On macOS every signal checks native UID and microsecond
process-start identity; Linux uses UID and kernel process-start ticks. Authority
exists only in the live supervisor's memory. Boot and launch identity remain in
the existing durable execution record, and restored records are never signalled.
No scan by process name, user-wide ownership, or working-directory inference is
used. Unrelated native daemons outside the owned ancestry are excluded.

Before termination, verified SIGSTOP freezes the leader. Bounded ancestry
inventories discover and freeze each child generation, verify the OS reports
all surviving members stopped, and require a second inventory to add no new
members. TERM remains pending while processes are frozen; the existing 500 ms
escalation sends KILL without reopening a fork/reparent gap. Stop is forceful:
native tool shutdown hooks and unflushed native conversation data are not
promised. Use the native CLI's exit action when graceful native shutdown is
wanted. PTY capture still drains its existing bounded FIFO and seals history
only after EOF and durable tail completion.

The process inventory is bounded to 8192 entries, 1 MiB and 500 ms; retained
owned identities are capped at 1024 per session, and closure has a 500 ms budget
within the existing two-second lifecycle deadline. A process inventory failure,
identity inspection error, or exceeded bound reports StopFailed and retains the
child handle and known identities, including orphaned children, for retry.
An inventory gap cannot clear after the verified leader disappears. A failed
attempt cannot be overwritten by ordinary native-exit handling. Successful
completion drops the tracker so a stopped session incurs no polling cost.

The live tracker samples ancestry every 100 ms. Deliberately double-forking and
reparenting between samples before Stop begins is outside this ownership proof;
crash recovery keeps its conservative same-boot descendant uncertainty. Stop's
freeze closure specifically handles ordinary children forking into separate
process groups while Stop is requested. This does not claim a kernel sandbox
against malicious process escape or remove the operating system's interval
between identity inspection and signal delivery.

Implementation validation and fresh native receipts are recorded below after
running against the final integration revision.

## Verification checkpoint

On macOS, locked build, formatting and warnings-denied all-target Clippy passed.
The escalated all-target run at 240f503 passed 26 unit checks (one subprocess
probe ignored directly and driven by terminal restoration) and 50 ownership
checks, with two measurement scenarios ignored. It exposed one failing proof
assertion: captured bytes exceeded *post-write* fixture-ledger receipts. The
exact saturation rerun reproduced it in 3.81 seconds. Freezing can interrupt the
producer after its PTY write (or a partial write), before the corresponding
ledger receipt. This is not a bound on accepted bytes, whose exact expected
prefix remains asserted independently. The corrected proof allows exactly one
in-flight 128-byte fixture write and records the receipt difference as signed,
instead of the former unsigned assumption. Its green rerun took 3.54 seconds:
completed receipts were 9856 bytes, captured fixture prefix 9984 bytes,
`emitted_not_captured=-128`, and the in-flight cap 128. All lifecycle, final
repair, FIFO retention and no-owned-coding assertions passed.

After merging a39308b and refining parent identity checks, the final focused
`stop_quit_and_native_exit_terminate_quiet_foreground_tool_in_separate_process_group`
passed in 5.54 seconds. It covers explicit Stop, ordinary quit, and natural root
exit with a previously observed separate-group child; each stops the heartbeat
and process, preserving the unrelated sentinel. The final-storage commit/quit
retry check passed in 1.78 seconds. Refresh also rechecks the parent's full live
identity at the actual adoption boundary, and a UID change with the same process
start identity fails closed rather than treating that process as absent.

The initial full run inside the command sandbox hit prelaunch supervisor
acquisition failures and was interrupted; these do not establish a lifecycle
failure. The escalated real-process runs above are the reported regression
checks. The broad checkpoint run failed before later integration binaries; the
root evidence owner runs final union checks and native receipts. This ticket
does not label the milestone complete.

Raw inventory-command wall samples are in
[18-child-scope-inventory-wall-time.json](18-child-scope-inventory-wall-time.json).
Forty executions observed 808 processes / 9696 output bytes, averaging 13.54 ms
and peaking at 18.59 ms. These are elapsed command wall times, **not CPU
utilization**. Background sampling was reduced from 20 ms to 100 ms; explicit
Stop performs its own fresh freeze/inventory closure independently of that
background interval. Full resource/latency revalidation belongs to the final
tracker measurement run.

Final child-stop source 6263726: locked build passed. Focused scripted-session
integration passed (29.09 seconds), followed by the styled-screen/resize/focus/
reconnect integration (8.91 seconds). The child-stop branch is clean; subsequent
native receipts, final resource measurement and union all-target run are owned
by ticket 18 and the integration coordinator.
