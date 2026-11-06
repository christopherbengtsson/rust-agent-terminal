# 02: Retain ownership through partial spawn failure

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** A cancelled or failed launch never abandons a process merely because setup failed after spawn.

- [ ] Install process/resource ownership immediately after successful spawn, before subsequent fallible setup. Keep gate, scope, PTY and capture obligations owned until transferred or confirmed cleaned.
- [ ] Exercise failures after creation and before input/capture/authorization installation, plus cancellation while creation is outstanding, with actual gated fixture processes.
- [ ] Never release a cancelled gate. Unconfirmed cleanup preserves a retryable reservation and blocks replacement rather than dropping the child or inventing success.
- [ ] Keep Stop responsive during storage trouble, retain original native exit outcomes and preserve current descendant verification. Do not replace proven lifecycle or recovery implementations.
- [ ] Preserve ordinary launch, native argument/environment/cwd construction, missing executable and quit behavior. Record fixture checks separately from native compatibility.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
