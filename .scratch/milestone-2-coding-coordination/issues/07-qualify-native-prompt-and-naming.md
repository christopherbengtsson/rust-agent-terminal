# 07: Qualify prompt-led native work and naming

Status: ready-for-agent
Resolved: no
Blocked by: 03, 05

**What to build:** Explicitly start an adopted workspace with an optional first prompt and obtain a truthful readable, editable label with both native tools.

- [ ] Demonstrate supported first-prompt invocation with real Claude and Codex while retaining native prompts/approvals, Ctrl-C and managed process ownership. Do not inject an extra hidden coding turn.
- [ ] Derive readable names from the supplied work, with a stable generated fallback and manual rename. Keep workspace/folder/native conversation identity independent of title or branch changes.
- [ ] Use native conversation titles only with reliable attribution; no undisclosed AI provider, raw-key/password recording or screen-text guess may invent native title ownership.
- [ ] Create without starting, restore and reconciliation never execute an agent or submit a prompt. Queue/cancellation/failure behavior preserves the explicit initial launch intent without duplicate turns.
- [ ] Record actual native versions and evidence for prompt delivery/naming, unsupported cases and fallback behavior. Return an evidence-backed decision if feasibility changes the accepted experience.

## Comments

Readable history is qualified independently in ticket 08; this slice covers
native prompt delivery and naming.
