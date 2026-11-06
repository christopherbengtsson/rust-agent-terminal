# 07: Qualify prompt-led native work and naming

Status: ready-for-agent
Resolved: yes
Blocked by: 03, 05

**What to build:** Explicitly start an adopted workspace with an optional first prompt and obtain a truthful readable, editable label with both native tools.

- [x] Demonstrate supported first-prompt invocation with real Claude and Codex while retaining native prompts/approvals, Ctrl-C and managed process ownership. Do not inject an extra hidden coding turn.
- [x] Derive readable names from the supplied work, with a stable generated fallback and manual rename. Keep workspace/folder/native conversation identity independent of title or branch changes.
- [x] Use native conversation titles only with reliable attribution; no undisclosed AI provider, raw-key/password recording or screen-text guess may invent native title ownership.
- [x] Create without starting, restore and reconciliation never execute an agent or submit a prompt. Queue/cancellation/failure behavior preserves the explicit initial launch intent without duplicate turns.
- [x] Record actual native versions and evidence for prompt delivery/naming, unsupported cases and fallback behavior. Return an evidence-backed decision if feasibility changes the accepted experience.

## Comments

Readable history is qualified independently in ticket 08; this slice covers
native prompt delivery and naming.


Implemented and scoped acceptance qualified on 2026-11-06. The
[versioned proof](../../../docs/proofs/m2-07-prompt-naming.md) records actual
Claude Code 2.1.319 / Codex CLI 0.167.0 first-prompt delivery, native question /
one-time approval / Ctrl-C, stable identities, durable manual rename, stopped
restore without execution, and focused queue-cancellation / normal-small UI
checks. Labels are truthful local prompt defaults or stable generated fallbacks;
no native title ownership is guessed. This ticket starts adopted workspaces;
Create/setup controls remain the subsequent creation ticket's responsibility.

The combined aggregate all-target gate remains separately pending; this ticket
resolution does not relabel the earlier missing-checkout display failure or
claim that aggregate passed. Ticket 06 owns the focused correction and the
final merged branch must run the complete required checks.
