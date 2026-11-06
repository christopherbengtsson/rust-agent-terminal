# Milestone 2 ticket breakdown

Status: ready-for-agent

Spec: [coding coordination](spec.md). The user approved the ticket granularity
and blocking edges. All 19 tickets are published under issues as ready-for-agent;
completion remains separate and every ticket is currently unresolved. Start only
when its listed blockers are resolved.

The first three are narrowly verifiable prefactoring slices; no separate refactor
milestone is introduced. Other slices deliver a complete usable path for their
stated scenario. Independent tickets have no artificial dependencies merely
because they may touch a common Rust module; implementation coordination can
serialize edits without misrepresenting the dependency graph.

Review refinements: prompt/naming and readable-history qualification are separate
slices; Whole workspace inspection can start on adopted branches without creation;
folder removal verifies recovery of the exact current HEAD even before archive.

1. **[Correlate durable session operations](issues/01-correlate-durable-session-operations.md)** — Blocked by: None. Starting, stopping and retrying the existing single session keep the same behavior, while only the exact matching durable completion can settle its launch outcome.

2. **[Retain ownership through partial spawn failure](issues/02-retain-partially-created-execution.md)** — Blocked by: None. A cancelled or failed launch never abandons a process merely because setup failed after spawn.

3. **[Deepen one Agent session runtime](issues/03-deepen-agent-session-ownership.md)** — Blocked by: 01, 02. The existing one-session workflow keeps working through one concrete session owner that handles provisional launch, native controls, stop and final history completion.

4. **[Resolve worktrees through bounded execution-owned jobs](issues/04-worktree-jobs-and-identity.md)** — Blocked by: None. Discovery, refresh and adoption resolve actual checkouts without blocking navigation or shared session controls.

5. **[Register projects and retain workspace identity](issues/05-retain-projects-and-workspaces.md)** — Blocked by: 01, 04. Add existing local repositories, navigate their discovered/adopted workspaces and reopen the dashboard to the same retained work and history.

6. **[Run independent workspaces without mixed controls or history](issues/06-coordinate-concurrent-workspaces.md)** — Blocked by: 03, 05. Launch coding agents in separate workspaces and switch between them while each retains its own input, screen, status, lifecycle and captured history.

7. **[Qualify prompt-led native work and naming](issues/07-qualify-native-prompt-and-naming.md)** — Blocked by: 03, 05. Explicitly start an adopted workspace with an optional first prompt and obtain a truthful readable, editable label with both native tools.

8. **[Qualify readable native session history](issues/08-qualify-readable-native-history.md)** — Blocked by: 03, 05. Open retained history as useful readable text with stable passage locations, independently of native resume and automatic naming.

9. **[Create and start an independent workspace](issues/09-create-and-start-workspaces.md)** — Blocked by: 06, 07. Create a new branch and folder from the chosen base and optionally start the selected agent with the supplied prompt in one explicit flow.

10. **[Run configured setup with recoverable failure choices](issues/10-prepare-new-workspaces.md)** — Blocked by: 09. New workspaces run the project setup command with visible progress before a requested launch, and retain useful recovery choices if it fails.

11. **[Browse and search retained workspaces and conversations](issues/11-find-workspaces-and-history.md)** — Blocked by: 06, 08. Find work by metadata or words in saved history across projects and open the selected chat at its matching passage.

12. **[Resume the explicitly selected native conversation](issues/12-resume-selected-conversations.md)** — Blocked by: 11. Choose an older or latest conversation in a workspace and explicitly recover it through its original native tool.

13. **[Inspect all working changes in the dashboard](issues/13-inspect-working-changes.md)** — Blocked by: 05. Open a workspace diff and inspect staged, unstaged and new files with keyboard file navigation and visible comparison freshness.

14. **[Inspect prior commits and current workspace changes](issues/14-inspect-whole-workspace.md)** — Blocked by: 13. Switch explicitly to Whole workspace and see branch work from its selected base alongside current edits.

15. **[Inspect diffs and edit source with reliable editor return](issues/15-inspect-and-edit-externally.md)** — Blocked by: 14. Open either comparison in Vim or a configured editor, open real workspace files for edits, and return to the same dashboard context.

16. **[Stop, save and archive a workspace](issues/16-checkpoint-and-archive.md)** — Blocked by: 10. Stop and archive saves unfinished code locally, reports exactly what was saved and hides the workspace without deleting its folder or chats.

17. **[Clean local branches while preserving archive recovery](issues/17-clean-branches-and-restore.md)** — Blocked by: 16. Archiving cleans app-created local branch names by default, and returning to archived work safely restores a usable branch.

18. **[Remove eligible workspace folders without losing saved work](issues/18-remove-workspace-folders.md)** — Blocked by: 17. Preview and explicitly remove an eligible linked workspace folder while preserving code recovery, history and unrelated repository state.

19. **[Complete the Milestone 2 coordination evidence](issues/19-integrated-native-coordination-proof.md)** — Blocked by: 12, 15, 18. Demonstrate C1–C7 together with both native agents, editors, concurrent projects and honest recovery/resource evidence on macOS.

## Approval

2026-11-06: the user approved the proposed breakdown ("great"). Published all
19 tickets with their reviewed blocking edges. No implementation has started.
