# 09: Create and start an independent workspace

Status: ready-for-agent
Resolved: no
Blocked by: 06, 07

**What to build:** Create a new branch and folder from the chosen base and optionally start the selected agent with the supplied prompt in one explicit flow.

- [ ] Offer Create and start and Create without starting. No manually entered title is required; show project/base/agent and collision-safe suggested branch/folder with overrides.
- [ ] Normally fetch configured remote/base, retain exact ref/commit used and allow explicit base override. Explain fetch failure and offer explicit local-code fallback; support repositories without remotes.
- [ ] Use stable product-managed per-project folders outside the original checkout; never overwrite a branch or existing path. Original checkout/index and unrelated work remain unchanged.
- [ ] Persist attributed recoverable creation/launch phases; lost reply, cancellation, partial Git success or failed agent start retains truthful state and cannot duplicate branches/folders/agents on retry.
- [ ] Verify real repositories including default-branch ambiguity, collisions, remote failure, late/cancelled completion and creation alongside another running workspace. Expose only implemented choices.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
