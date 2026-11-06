# 13: Inspect all working changes in the dashboard

Status: ready-for-agent
Resolved: yes
Blocked by: 05

**What to build:** Open a workspace diff and inspect staged, unstaged and new files with keyboard file navigation and visible comparison freshness.

- [x] Working is the default every time inspection opens. Distinguish staged/unstaged/new-file state, including overlapping edits, new files, renames, deletions, binaries and conflicts.
- [x] Generate comparisons in execution-owned bounded workers without changing the user index, running arbitrary external diff commands or blocking input/Stop.
- [x] Label workspace/comparison identity and freshness; refreshed results preserve selected file/context where possible and stale responses cannot replace another workspace view.
- [x] Use safe printable display and bounded file/content presentation. Explain unsupported, submodule, missing-file and oversized cases instead of silently omitting changed work.
- [x] Verify actual Git repositories and dashboard rendering/keyboard behavior while an agent changes files, including normal and small terminal sizes.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.

Implemented in `feature/m2-ticket13` from integration `b74faa31`, with the final
concurrent-session and readable-history integrations merged before completion.
Working inspection is supervisor/workspace-owned, read-only, cancellable and
bounded; it preserves byte paths, request attribution and selected context.

Evidence: [Working inspection proof](../../../docs/proofs/m2-13-working-diff.md)
records five actual-Git/dashboard seam tests, the Controller busy/timeout/targeted
Stop and envelope-rejection proof, and inspected actual 160×40 / 60×18 dashboard
PTY captures while external edits change the shared files. Build, fmt, clippy,
21 App and 13 UI tests passed after both integrations. The actual diff PTY proof
uses controlled external file writes and no coding child; it makes no additional
native Claude/Codex or native terminal-emulator compatibility claim. Whole
workspace (14), editor handoff (15) and pinned PR Review remain separate.
