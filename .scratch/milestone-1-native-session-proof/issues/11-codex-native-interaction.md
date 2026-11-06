# 11: Demonstrate Codex CLI interaction on macOS

Status: ready-for-agent
Blocked by: 05
Resolved: yes

**What to build:** Use Codex CLI as another working native agent choice through the same dashboard interaction, using existing authentication and checkout context.

- [x] Select Codex CLI explicitly and launch directly in the adopted checkout. Missing executable, authentication problems and spawn failure produce actionable guidance and preserve dashboard context.
- [x] On recorded macOS/Codex versions, answer real input and approval prompts, complete a turn, use native Ctrl-C, escape to navigation and reattach to the same child/conversation.
- [x] Exercise repeated resize, Unicode/paste and relevant native terminal modes; demonstrate that the documented escape chord works without silently intercepting ordinary Codex shortcuts.
- [x] Use existing authentication without copying credentials into product records. Changing tool choice starts a new native conversation, never pretends to transfer Claude Code context.
- [x] Record reproducible native steps, versions, results and limitations separately from fixture checks. An inability to complete the native interaction remains a support-gate failure.
- [x] Keep launch and focus behavior consistent with the existing session contract, and update working Codex instructions. Reliable semantic status is delivered in the separate Codex status ticket.

## Comments

Implemented explicit `x` Codex CLI launch through the shared supervisor request,
worktree exclusion and durable outcome path; uses direct `codex --no-daemon` in
the adopted checkout with existing authentication. [Native macOS evidence](../../../docs/proofs/11-codex-native.md)
records Codex 0.167.0 binary hash, actual conversational question/answer, one-time
command approval, Unicode bracketed paste, completed turns, active-turn Ctrl-C,
repeated resize, same-process reattach, native exit and confirmed proof-supervisor
shutdown. Fixture checks are separated from real CLI observations. Authentication
failure was not forced by changing credentials; native auth UI plus `codex login`
guidance are documented. This resolves native interaction only; ticket 12 still
owns semantic status evidence, which remains unknown here. Mouse forwarding,
graphical terminal apps, enhanced protocols and native resume are unverified.
