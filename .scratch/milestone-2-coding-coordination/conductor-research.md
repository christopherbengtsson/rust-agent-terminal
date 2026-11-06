# Conductor reference research for Milestone 2

Researched: 2026-11-06. Scope: local coding coordination. Primary sources are Conductor's official documentation and changelog, reached from [conductor.build](https://www.conductor.build/). This is research, not an implementation spec or a record of product decisions. No app was installed or exercised. Documentation establishes documented behavior; unknowns below are not claims that a feature is absent.

## Registration and existing work

**Documented:** Conductor offers Open project for an existing local repository, Open GitHub project, and Quick start for a new repository. Adding a repository creates a workspace. Its initial files are tracked Git files; ignored configuration and dependencies need setup or copying. Starting a chat is described as a subsequent action. [First workspace](https://www.conductor.build/docs/first-workspace)

**Documented:** Conductor creates a Git worktree for each workspace. The original checkout remains the project root; the usual workspace path is `~/conductor/workspaces/<repo name>/<workspace name>`. Worktrees share Git objects, refs and remotes. Workspace creation therefore does not clone the repository again. [Git worktrees](https://www.conductor.build/docs/concepts/git-worktrees)

**Unknown:** The sources reviewed do not establish discovery and adoption of arbitrary existing worktrees, registration identity/deduplication rules, or exact GitHub clone destination and clone flow. Starting from an existing branch is documented, but that does not prove adoption of a checkout already using it.

**Terminal-product recommendation:** Keep the existing requirement to register local repositories, discover worktrees without mutation, and explicitly adopt a checkout. Cloning through ordinary Git and adoption are existing product choices, rather than behavior to attribute to Conductor.

## Workspace, task, session and creation flow

**Documented:** A Conductor project holds one repository and its settings/workspaces. A workspace represents one task stream with one branch and working tree; it can run multiple processes. The branch identifies the work; the workspace directory is secondary. [Isolated workspaces](https://www.conductor.build/docs/concepts/workspaces-and-branches)

**Documented:** Independent changes use separate workspaces. Multiple agents can instead share one workspace when they need the same branch, files and context; the documented tradeoff is concurrent edits to the same files. [Parallel agents](https://www.conductor.build/docs/concepts/parallel-agents)

**Documented:** Workspace setup can run a script when the workspace is created, before agent coding. Local ignored files need deliberate handling. [Project configuration](https://www.conductor.build/docs/configure-your-project)

**Terminal-product recommendation:** Map Conductor's workspace to this product's task plus associated worktree, keeping sessions beneath that task. Preserve this release's one active coding session per worktree. Do not import Conductor's simultaneous shared-checkout agents merely because the reference permits them. A creation flow can gather project, task label, base and agent, create the worktree, then launch explicitly; exact screen order is a proposed terminal design.

**Unknown:** The reviewed docs do not completely specify every creation field, generated-name collision policy, cancellation/rollback boundary, or whether submitting every variant of the new-workspace composer also starts an agent. They establish workspace creation and chat start as distinguishable concepts.

## Base branch and fetch policy

**Documented:** A new workspace gets a new branch based on the repository's configured base (example: `origin/main`). Conductor fetches `origin` first so the workspace starts at the latest remote commit. The first chat instructs the agent to rename the generated branch to fit the work. Existing-branch creation is available through the Branches tab. Git prevents one branch being checked out in two worktrees. [Isolated workspaces](https://www.conductor.build/docs/concepts/workspaces-and-branches)

**Documented:** The workspace composer permits changing the target branch through the three dots next to the repository. [0.71.0 changelog](https://www.conductor.build/changelog/0.71.0-workspace-and-chat-links)

**Unknown:** The docs reviewed do not define offline fallback, fetch failure behavior, repositories without `origin`, exact default-branch discovery, or a guarantee that workspace creation never changes any other local refs.

**Terminal-product recommendation:** Treat fetch-before-create as a Conductor reference point, then explicitly decide failure and offline behavior. Record the selected base identity and actual commit used. Do not let an implied network refresh silently substitute a different base. Existing product requirements for offline coding and local repositories need a policy beyond these Conductor docs.

## Finding work and past context

**Documented:** The workspace-history page can open existing workspaces or unarchive old ones, with filters for repository, branch or PR number. [0.28.0 changelog](https://www.conductor.build/changelog/0.28.0-workspaces-page-claude-s-context-interactive-planning-keyboard-nav-and-more)

**Documented:** Workspace search, filters and recently visited workspaces are available in the command palette. [0.77.0 changelog](https://www.conductor.build/changelog/0.77.0-early-access-multiplayer-api-background-tasks-performance)

**Documented:** Inside a workspace, command-palette search finds that workspace's chats, including closed ones. [Current changelog](https://www.conductor.build/changelog)

**Documented:** Search within the current chat uses Command-F. [Chat-search release](https://www.conductor.build/changelog/0.31.0-search-chats-add-scripts-in-the-ui-and-view-setup-script-logs)

**Unknown:** These sources do not establish one local UI search that scans all transcript text across every project and archived task. Cloud API transcript SQL search is a separate capability and cannot establish this local UI behavior.

**Terminal-product recommendation:** Distinguish finding a task/workspace by metadata from searching retained conversation content. Preserve the product requirement that archived tasks and stopped sessions remain discoverable. Search scope, included fields, result snippets, archive inclusion and navigation destination still need explicit local decisions.

## Archive and recovery boundaries

**Documented:** The History pane restores archived workspaces including saved chat history. [Workflow](https://www.conductor.build/docs/concepts/workflow)

**Documented:** If a checkout was moved or deleted externally, restoration can fail; the suggested continuation is a new workspace from the branch. Removing a repository is documented as deleting its workspaces and chats. [Troubleshooting](https://www.conductor.build/docs/troubleshooting/issues)

**Documented:** Settings can delete a workspace branch on archive or archive automatically on merge. [Settings reference](https://www.conductor.build/docs/reference/settings/reference)

**Terminal-product recommendation:** Keep this product's explicit archive, worktree removal and history-deletion operations separate. Its existing safety/retention requirements are stronger than the behavior demonstrated by these sources. Do not infer native CLI resume guarantees from Conductor chat restoration.

## Diff inspection and external editing

**Documented:** Conductor has a built-in Diff Viewer with changed-file navigation, unified layout and commit filtering. [Diff viewer](https://www.conductor.build/docs/reference/diff-viewer)

**Documented:** Keyboard controls distinguish Open in app, the diff view, Show uncommitted changes, file edit mode, quick-open files, file-content search and chat search. [Keyboard shortcuts](https://www.conductor.build/docs/reference/keyboard-shortcuts)

**Documented:** The workflow permits opening the workspace in an IDE. [Workflow](https://www.conductor.build/docs/concepts/workflow) The 0.71 release says open-in buttons open the file currently being viewed. [0.71.0 changelog](https://www.conductor.build/changelog/0.71.0-workspace-and-chat-links)

**Unknown:** The reviewed sources do not settle the Diff Viewer's initial/default Git comparison, precise treatment of staged/unstaged/untracked files, merge-base computation, external diff command protocol, or editor-exit/dashboard-focus restoration semantics.

**Terminal-product recommendation:** Conductor separates change inspection from opening editable workspace files. Therefore a proposal that only hands an immutable patch/snapshot to an editor is not sufficient evidence of mimicking its editor workflow. Preserve this product's specified working-changes default and explicit whole-task alternative; decide how Vim or another configured editor shows comparisons and lets the user reach the real worktree files. State comparison context and return behavior explicitly. This implementation choice remains local to the terminal product.

## Follow-up: titles, branch names and stable directories

**Documented:** Conductor's 0.20.0 release introduced automatically generated chat-tab titles from the first message, using Gemini, with an opt-out through Strict data privacy. This is historical evidence of app-managed title generation, not evidence of adopting a title emitted by native Codex or Claude. [0.20.0 changelog](https://www.conductor.build/changelog/0.20.0-file-picker-and-chat-titles) Its privacy notice still describes an optional feature that sends the first chat message to a third-party provider to generate a session summary title. It does not identify the current provider. [Privacy notice](https://www.conductor.build/privacy)

**Documented:** Every workspace gets a city name providing a stable directory base; the sidebar primarily identifies active work by branch name or PR title. These are distinct roles. [Cities](https://www.conductor.build/docs/reference/cities)

**Documented:** The 0.89.0 release says new workspaces receive descriptive branch names based on the work and exposes an Automatically rename new branches toggle under Settings → Git. [0.89.0 changelog](https://www.conductor.build/changelog/0.89.0-sign-in-with-chatgpt) The workspace concept documentation describes instructing the agent to rename the generated branch on first chat, as recorded above. The exact present implementation behind the newer toggle is not established by these sources.

**Unknown:** No reviewed local-desktop source establishes a required manually entered title before creation, or use of native Codex/Claude-generated chat titles as Conductor's source of truth. The cloud API explicitly permits unnamed workspaces with a random city fallback, but that does not settle the local composer contract. [API](https://www.conductor.build/docs/api)

**Terminal-product recommendation:** To mimic the documented experience, let the developer provide the task prompt, give the checkout a stable generated location immediately, and derive a readable task/chat title from the prompt with later renaming available. Getting that title from the selected native agent is a possible implementation decision, rather than a documented Conductor requirement. Keep task identity, editable title, branch name, directory and native conversation identity separate. Do not require a second manual title merely to make a new task identifiable.

**Fetch follow-up:** No readily located official source settled offline creation or fetch-failure fallback. The explicit fetch-before-create behavior and the unknowns in the base-branch section remain the research boundary.

## Q13 evidence: archive, deletion and protection of local work

**Documented historical behavior:** The 0.33.5 release says archive automatically saves Git state, including uncommitted files, and permits unarchiving even after the original branch changes. It also says archiving interrupts a setup script. [Better archiving](https://www.conductor.build/changelog/0.33.5-better-archiving)

**Documented historical behavior:** The 0.44.0 release explicitly says archiving saves uncommitted changes as a commit. Therefore a policy of simply refusing to archive every dirty checkout is not the behavior established by this Conductor source. [0.44.0 changelog](https://www.conductor.build/changelog/0.44.0-new-sidebar-rebuilt-composer-codex-checkpoints)

**Documented:** `git.delete_branch_on_archive` controls deletion of the workspace branch, and `git.archive_on_merge` controls automatic archive after merge. The reference lists these options without specifying defaults or dirty/unpushed checks. [Settings reference](https://www.conductor.build/docs/reference/settings/reference)

**Documented:** History restoration includes saved workspace state and chats; removing a repository deletes its workspaces and chats, as recorded above. [Troubleshooting](https://www.conductor.build/docs/troubleshooting/issues)

**Documented later behavior:** A later release permits archiving running chats directly from the sidebar. This supersedes any assumption based only on the older release that blocked archive while agents ran. [Changelog page 5, Force archive](https://www.conductor.build/changelog/page/5)

**Unknown:** The reviewed official sources do not establish an explicit unpushed-commit safety gate, treatment of all untracked/ignored files during archive snapshots, snapshot ref/commit lifetime, local branch-deletion default, permanent individual-workspace/history-deletion contract, or a precise current archive confirmation screen. Saving dirty work as a commit does not prove that every ignored resource or every later deletion is recoverable. No terminal-product policy is decided by this evidence.

## Q17 evidence: local setup scripts and failed setup

**Documented:** A configured `scripts.setup` runs after creating a workspace and checking out tracked files. Scripts execute with the workspace as their working directory. Configuration is available through project Scripts settings or `.conductor/settings.toml`; generated files, dependencies and symlinks are example uses. `CONDUCTOR_ROOT_PATH` permits deliberate access to root-checkout files. Run actions instead start through the Run button. [Scripts](https://www.conductor.build/docs/reference/scripts)

**Documented:** The shared-project configuration guide uses a project-specific command such as `pnpm install`; this is an example, not a universal built-in install command. Conductor's configuration assistant prompt asks for commands suited to the existing repository and recommends local overrides unless shared settings are requested. [Configure your project](https://www.conductor.build/docs/configure-your-project)

**Documented:** Failed-setup troubleshooting lists missing ignored files, dependencies installed only in the root checkout, fixed paths and unavailable authentication as common causes. Archive-script failure may prevent archive completing; the documented recovery is fixing the command and trying again. [Troubleshooting](https://www.conductor.build/docs/troubleshooting/issues)

**Documented:** Local shell-environment capture has a five-second startup limit and can abort; most setup/run commands use zsh. [Shell configuration](https://www.conductor.build/docs/reference/shells)

**Unknown:** No reviewed source specifies a local automatic setup retry count/backoff, exact retry control, whether coding launch is blocked after setup failure, default behavior with no configured script, per-creation confirmation for configured scripts, or a fresh agent's obligation to repair failed setup. Cloud build/retry behavior is not evidence of local setup semantics. These remain questions requiring a product choice or direct app validation, rather than assumptions attributable to Conductor.

## Q14–Q16 follow-up evidence: diff defaults, freshness and editor location

**Q14 documented:** In addition to the previously cited diff controls, the Git panel retains the Uncommitted/commit filter when switching among All files, Changes and Checks. This establishes retained filtering across these tabs, rather than a guaranteed reset to working changes on each inspection. It does not identify the initial comparison or Git comparison algorithm. [Changelog page 5](https://www.conductor.build/changelog/page/5)

**Q15 documented:** The current changelog mentions a loading bar when diffs refresh and a correction for stale diffs during simultaneous user/agent editing. [Current changelog](https://www.conductor.build/changelog) Expanded diff context is retained when files update. [Changelog page 3](https://www.conductor.build/changelog/page/3) These support refreshed comparisons; they do not establish exact refresh triggers, timing, atomicity, or behavior while an external editor is active. A blanket claim that Conductor uses only manually refreshed snapshots is unsupported.

**Q16 documented:** Open-in buttons open the file currently being viewed. [0.71.0 changelog](https://www.conductor.build/changelog/0.71.0-workspace-and-chat-links) Opening at the exact current external-editor line/column, its command protocol and its return-to-dashboard semantics remain unproven by the reviewed official sources.

## Q19 targeted follow-up: removing the checkout with local commits

The targeted official-source search did **not** settle whether local archive always removes the entire worktree directory, or what happens specifically when a workspace contains unpushed commits. The strongest established archive behavior is preservation of Git state, including uncommitted files, and restoration even when the original branch changed. [0.33.5](https://www.conductor.build/changelog/0.33.5-better-archiving) Later wording explicitly calls the saved uncommitted changes a commit. [0.49.0](https://www.conductor.build/changelog/0.49.0-conductor-allegro-gpt-5-5)

The `git.delete_branch_on_archive` option establishes that deleting the local branch is a separately configurable part of archive; it does not establish its default, whether unpushed commits block it, or which additional ref retains saved work. [Settings reference](https://www.conductor.build/docs/reference/settings/reference) A fix for archive cleanup failing to remove generated files supports cleanup of artifacts, but does not prove the exact worktree-folder removal contract. [Changelog page 5](https://www.conductor.build/changelog/page/5)

Repository Remove is explicitly documented to delete all workspaces and chats for that repository. This must not be equated with the restorable Archive operation. [Troubleshooting](https://www.conductor.build/docs/troubleshooting/issues) The reviewed sources do not establish a separate permanent individual-workspace-delete contract, its unpushed checks, or its retention guarantees.

**Answer boundary:** It is supported to say Conductor archives recoverable saved work and can optionally delete the branch. It is unsupported to say it always keeps the branch, always removes/keeps the folder, automatically pushes local commits, requires a push before removal, or preserves every local commit indefinitely through a documented checkpoint/ref. Native app validation or a more explicit first-party contract would be needed for those specifics.
