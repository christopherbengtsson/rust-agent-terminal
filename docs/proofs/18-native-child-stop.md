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

The live tracker samples ancestry every 20 ms. Deliberately double-forking and
reparenting between samples before Stop begins is outside this ownership proof;
crash recovery keeps its conservative same-boot descendant uncertainty. Stop's
freeze closure specifically handles ordinary children forking into separate
process groups while Stop is requested. This does not claim a kernel sandbox
against malicious process escape or remove the operating system's interval
between identity inspection and signal delivery.

Implementation validation and fresh native receipts are recorded below after
running against the final integration revision.
