# 12: Resume the explicitly selected native conversation

Status: ready-for-agent
Resolved: no
Blocked by: 11

**What to build:** Choose an older or latest conversation in a workspace and explicitly recover it through its original native tool.

- [ ] Resume targets retained tool/conversation identity, not current screen or history index; create a new managed history segment under the same workspace and native conversation.
- [ ] An occupied worktree offers attach; another conversation requires confirmed stop and settled prior capture. Preserve canonical checkout/uncertain-execution reservations.
- [ ] Explain missing checkout, unsupported/unavailable native recovery and missing IDs; preserve readable history and offer an explicit new conversation. Never transfer native state between tools.
- [ ] Resume does not rewind Git. Restore/startup/reconciliation never submit a prompt or automatically resume, and same-request recovery never duplicates native execution.
- [ ] Verify selected older/newer conversations and tool switches with fixtures and actual Claude/Codex native markers; record exact versions and original-ID evidence.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
