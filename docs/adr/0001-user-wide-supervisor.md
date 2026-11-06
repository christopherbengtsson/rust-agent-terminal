# One supervisor and one connected dashboard per macOS user

Accepted during the architecture interview on 2026-11-06. One local supervisor
per macOS user owns managed sessions across projects, with at most one connected
dashboard controlling it at a time; a second dashboard explains the existing
connection rather than competing for control. This centralizes session launch
deduplication and worktree exclusivity, and avoids conflicting input, resize,
and quit commands at the cost of simultaneous dashboard access.

Detached activity survives dashboard loss, and reconnect becomes available when
the previous client disconnects or dies. Supervisor election, client-loss
detection, command retry semantics, and worktree identity remain to be resolved
in the interview and validated in the native-session proof.
