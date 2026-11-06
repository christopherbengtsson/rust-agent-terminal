# Preserve session history and make storage cleanup explicit

Accepted during the architecture interview on 2026-11-06. Retain saved session
history until explicit deletion, warn before storage becomes critically low, and
offer cleanup showing session age, size, and task, suggesting oldest stopped
sessions first. Age and archival are not permission to delete: valuable older
context must not disappear automatically to make room for newer sessions.

Edits are acknowledged as saved only after durable storage succeeds. Streaming
history uses bounded buffers and batched persistence, targeting a durable flush
at least once per second under healthy storage. This is a measured operating
target, not a maximum crash-loss guarantee during storage failure; a crash can
lose unflushed output. Clean shutdown flushes pending history and reports failed
writes. Storage failures are visible; when buffers fill,
backpressure takes priority over silently discarding output. Stopping PTY reads
can block output producers but does not safely suspend all agent activity or
background commands. The dashboard remains responsive for recovery and explicit
session stop.

Storage, memory, and performance budgets must be measured during implementation.
Automatic retention is deferred; it would require a separate opt-in policy.
Cleanup is not guaranteed to recover sufficient disk space, so persistent
storage failures still need a defined recovery flow.
