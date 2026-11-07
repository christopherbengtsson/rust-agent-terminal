# 14: Edit review text in the external editor

Status: ready-for-agent
Resolved: no
Blocked by: 01, 13

**What to build:** Edit a comment or overall message in the configured external editor, return to the same dashboard context, and save exact text locally with recoverable failure handling.

**Acceptance coverage:** R4; editor-related R10 recovery.

## Acceptance criteria

- [ ] Freeze the review cycle, draft generation, text target/version and original text before opening the editor. Preserve terminal handoff and independent process ownership from the consolidated workflow.
- [ ] Successful changed text saves to the original target through the draft owner; unchanged text is a no-op. Preserve selection and validation choices, and perform no publication.
- [ ] Read text before artifact cleanup. Keep identifiable recovery material until the matching durable save acknowledgement or explicit discard, including recovery after restart and lost acknowledgements.
- [ ] Nonzero editor exit, missing/unreadable artifact and failed save leave the original committed draft intact and preserve available edits. Offer applicable retry/reopen/apply-recovered-text/discard actions; unsuccessful execution requires explicit apply.
- [ ] If newer text or a superseded draft would be overwritten, retain returned text and require explicit reconciliation. A PR-only comparison change still allows an original-context text save while preserving staleness.
- [ ] Editor completion is not a durable-save acknowledgement. Retry targets stay stable and cannot create duplicate original comments; stale/duplicate save responses cannot delete the recovery artifact early.
- [ ] Exercise controlled editor unchanged/changed/error/deleted-artifact cases and durable conflict/restart cases, plus installed macOS Vim and Pico handoff, save, resize, restoration and supervisor continuity. Preserve existing live-source and inspection behavior.

## Scope and handoff

Keep terminal/navigation ownership with the dashboard and durable review/version decisions with the draft owner. The editor module owns process/artifact lifecycle rather than becoming a GitHub or draft persistence module.

## Comments
