# 01: Deepen external-editor workflow

Status: ready-for-agent
Resolved: no
Blocked by: None

**What to build:** Preserve working comparison inspection and live-source editing while concentrating preparation, editor execution, artifact lifetime and completion in one external-editor workflow. This behavior-preserving prefactor makes recoverable review-comment editing possible without spreading another lifecycle across dashboard callers.

**Acceptance coverage:** Editor prerequisite for R4; preserve existing coding behavior.

## Acceptance criteria

- [ ] The dashboard opens Working and Whole workspace comparisons read-only and opens live source for explicit editing through the consolidated workflow, retaining existing positioning and configuration behavior.
- [ ] Preparation, process waits and artifact cleanup remain outside the UI loop. The dashboard retains terminal release/reacquisition and navigation context; the editor module owns its process and artifact lifecycle.
- [ ] Callers no longer coordinate separate preparation/running receivers or infer workflow outcomes from an inspection boolean. An added forwarding module without concentrating lifecycle knowledge does not satisfy this ticket.
- [ ] Editor launch failure, nonzero exit and worker disconnection restore the terminal and prior dashboard context with an accurate outcome; independently running coding sessions remain unaffected.
- [ ] Preserve the independent editor process group and existing native Ctrl-C, resize and dashboard-return behavior. Do not generalize the coding-session owner or introduce an editor plugin framework.
- [ ] Verify controlled-editor success/failure through the workflow interface and existing dashboard PTY coverage. Run both affected installed-editor qualifications on macOS, recording versions and any unmet gate.

## Scope and handoff

Keep this prefactor green before review-text editing builds on it. Exact editable scratch recovery and durable review saves arrive in ticket 14; inspection artifacts must not become editable review drafts here.

## Comments
