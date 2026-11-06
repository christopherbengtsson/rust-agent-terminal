# 09: Create and start an independent workspace

Status: ready-for-agent
Resolved: no
Blocked by: 06, 07

**What to build:** Create a new branch and folder from the chosen base and optionally start the selected agent with the supplied prompt in one explicit flow.

- [x] Offer Create and start and Create without starting. No manually entered title is required; show project/base/agent and collision-safe suggested branch/folder with overrides.
- [x] Normally fetch configured remote/base, retain exact ref/commit used and allow explicit base override. Explain fetch failure and offer explicit local-code fallback; support repositories without remotes.
- [x] Use stable product-managed per-project folders outside the original checkout; never overwrite a branch or existing path. Original checkout/index and unrelated work remain unchanged.
- [x] Persist attributed recoverable creation/launch phases; lost reply, cancellation, partial Git success or failed agent start retains truthful state and cannot duplicate branches/folders/agents on retry.
- [x] Verify real repositories including default-branch ambiguity, collisions, remote failure, late/cancelled completion and creation alongside another running workspace. Expose only implemented choices.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Implementation is ready on `feature/m2-09-create-workspace` in `/tmp/rat-m2-09`.
It includes the current Whole workspace integration (`ea7deea`) and corrected
compatibility checkpoint (`0690026`), plus current external-editor integration
(`438b75d`). This ticket intentionally remains
`Resolved: no` while reopened upstream tickets 06/08 await their frozen full gate.
A fresh merger can integrate this branch and close its criteria afterward; no
additional individual full aggregate is claimed.

[Evidence](../../../docs/proofs/m2-09-workspace-creation.md): six real-Git/coordinator
checks; App and normal/small rendering checks; Controller creation/restart,
fetch-fallback, persistence-failure and cancellation outcomes; and the final
3.95-second dashboard/PTY fixture covering Create and start alongside another
running workspace, failed native exec, same-request deduplication and targeted
cancellation after native handoff without stopping a later conversation. Git
branch/operation claims recover branch-only partial success. Every G path disarms
prior launch intent, including an initial write failure. Late or provisional
native results remain hidden until that owner's durable acknowledgement.

Actual Claude 2.1.319 and Codex 0.167.0 startup/same-request/Stop smokes passed in
new folders; native folder-trust/hook-review gates prevented fresh initial-prompt
visibility. Exact argv/cwd is verified by the fixture, and the unchanged native
prompt adapter retains its ticket 07 qualification. No native prompt-turn,
physical-terminal, sleep/wake or reboot qualification is added. The extra isolated
creation-protocol guard case compiles and is delegated to the merger's combined
execution under the shared test-slot schedule. Configured setup remains ticket 10.
