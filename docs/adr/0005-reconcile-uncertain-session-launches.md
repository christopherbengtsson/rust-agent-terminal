# Recover launch results before retrying execution

Accepted during the architecture interview on 2026-11-06. A dashboard reconnect
recovers the result of the same session-launch request instead of issuing a new
launch when its acknowledgement was lost. After supervisor failure during
launch, show interrupted or uncertain state and establish whether a child
survived before allowing another agent in that worktree.

This favors preventing duplicate coding activity over immediately retrying an
uncertain launch. It requires stable launch-request identity and reconciliation;
it does not claim exactly-once process execution or permit automatic coding
restart. The prototype must exercise lost acknowledgements and crashes around
process creation before the behavior can be relied on.
