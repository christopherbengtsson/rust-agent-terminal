# 11: Demonstrate Codex CLI interaction on macOS

Status: ready-for-agent
Blocked by: 05

**What to build:** Use Codex CLI as another working native agent choice through the same dashboard interaction, using existing authentication and checkout context.

- [ ] Select Codex CLI explicitly and launch directly in the adopted checkout. Missing executable, authentication problems and spawn failure produce actionable guidance and preserve dashboard context.
- [ ] On recorded macOS/Codex versions, answer real input and approval prompts, complete a turn, use native Ctrl-C, escape to navigation and reattach to the same child/conversation.
- [ ] Exercise repeated resize, Unicode/paste and relevant native terminal modes; demonstrate that the documented escape chord works without silently intercepting ordinary Codex shortcuts.
- [ ] Use existing authentication without copying credentials into product records. Changing tool choice starts a new native conversation, never pretends to transfer Claude Code context.
- [ ] Record reproducible native steps, versions, results and limitations separately from fixture checks. An inability to complete the native interaction remains a support-gate failure.
- [ ] Keep launch and focus behavior consistent with the existing session contract, and update working Codex instructions. Reliable semantic status is delivered in the separate Codex status ticket.
