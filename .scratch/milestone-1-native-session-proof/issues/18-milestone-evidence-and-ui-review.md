# 18: Complete the milestone evidence and UI review

Status: ready-for-agent
Blocked by: 10, 12, 15, 16, 17

**What to build:** Hand over a complete, reviewable Milestone 1 proof with native compatibility, recovery, resource and visual evidence, and instructions describing demonstrated behavior.

- [x] Exercise the integrated one-repository adopted-worktree workflow with each real native tool: launch, attach, prompt/approval, Ctrl-C, focus escape, resize, native exit, stop, detach, quit and dashboard-loss reconnect. Verify no duplicate or automatic coding restarts.
- [x] Consolidate per-tool/version native status and recovery matrices, crash-window results, durable-history evidence, explicit cleanup outcomes and measured resource/latency results. Distinguish native evidence from fixtures and list every unresolved proof failure.
- [x] Capture normal and smaller terminal screenshots, including attached, attention, unknown and failure states. Compare against the reference image; show reviews as unavailable, one active session per worktree and the native terminal instead of a product message composer. Record material deviations, consequences and proposed alternatives.
- [x] Verify keyboard-only use, readable identity/focus/status without color alone, actionable errors and terminal restoration. No unimplemented controls or unsupported capability claims are introduced.
- [ ] Run stable Rust locked build, formatting check, Clippy for all targets with warnings denied, and all-target tests. Record checks actually run and unverified behavior; Linux CI cannot establish native macOS compatibility.
- [x] Align architecture and user instructions with working launch, tested versions, focus escape, detach/quit, recovery, retention, cleanup and limitations. No GitHub review publication or other later milestone functionality is added.
- [ ] Declare the milestone complete only when required native and measurement gates pass. Any failed or unavailable scenario stays explicit and blocks an unsupported completion claim; do not imply all release coding criteria or the combined workload have passed.

## Comments

2026-11-06: [Integrated evidence](../../../docs/proofs/18-milestone-evidence.md)
records real Claude318/Codex167 interaction, whole-dashboard captures, active
native supervisor failure and the passing pre-fix full all-target suite. An
ordinary approved Codex foreground command survived a reported successful Stop
in its separate default process group. This concrete native gate remains open;
a fresh ownership fix and real Stop plus ordinary-quit foreground-tool reruns
are required before resolving this ticket. No milestone-complete claim is made.

The correction and all four actual foreground Stop/ordinary-q retests passed on
a3842906 (exact hashes and collector-tail corrections are in the proof). Ticket
18 remains open for final code-followup checks, diagnosed aggregate failures and
resource/latency revalidation after the live-ownership sampling change.

Final runtime44b4d8c/17b9f65 was integrated in ticket18 at da46863c. All four
real native foreground Stop/q cases repeated end to end with one unchanged
prelaunch binary hash; independent same-identity readable History and final
Shutdown also passed for each. Exact receipts and source boundaries are in the
proof. Final resource revalidation and the complete final all-target suite
remain required before resolution.
