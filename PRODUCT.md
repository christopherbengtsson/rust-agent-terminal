# First usable release

## Purpose and decision status

[VISION.md](VISION.md) establishes the direction. This document defines the user-visible behavior and release requirements that make that direction usable for the first time. Detailed architecture belongs in a separate document.

The product is unnamed. Conductor is a reference application, not the product name. The [dashboard image](docs/images/conductor-dashboard-v3.jpg) is the UI target: reviews above a project/worktree/session tree on the left, a prominent active-session area on the right, and visible status and keyboard actions. Preserve its overall composition and visual direction as implementation progresses, adapting to terminal size. Raise feasibility problems and material deviations explicitly rather than silently substituting another UI. Branding, exact keybindings, commands, and example content remain illustrative.

The image shows multiple agents within a worktree and a simplified session transcript/input area. The first release still permits only one active coding session per worktree and preserves each agent's native CLI interaction. These differences must be visible in implementation reviews; the image does not override those confirmed behaviors.

The requirements below are confirmed product decisions. Deferred capabilities and unresolved details are listed separately at the end. This document defines intended behavior, not functionality already implemented in the repository.

## Target user, problems, and success

The first release serves an individual developer working on their own Mac, primarily through a shell, Git, an editor, and established coding agents. The initial reference workflow is the maintainer's daily work with Claude Code, with Codex CLI also supported at launch.

Three connected problems define the release:

- Parallel agent sessions become difficult to identify across terminal windows and projects.
- Ending or losing a terminal session makes its history and task context difficult to find again.
- Giving reviews and responding to feedback requires repeated switching between an agent and GitHub.

The release succeeds when the developer can find work needing attention, enter the correct session, inspect its changes, recover past context, and complete the next action from the terminal. Both core workflows must work together:

1. Coordinate local coding-agent sessions across projects and worktrees, including feedback and merge actions on task-linked PRs.
2. Discover GitHub review requests, prepare local drafts in the background, and let the developer review and explicitly publish them.

The tool itself is free and built in Rust. Agent subscriptions and API usage remain the developer's responsibility. The license remains undecided.

## Setup, tools, and configuration

The first release runs locally on macOS. There is no separate product account or sign-up service.

Setup checks for Git, GitHub CLI, the selected agent tools, and the configured editor. It reuses GitHub CLI authentication and each agent's existing account login or API-key configuration. Missing tools or authentication produce actionable setup guidance. The user can begin with one configured agent; both Claude Code and Codex CLI are supported product integrations.

Setup presents recommended defaults and explains that background review preparation uses the selected agent's account or API allowance. Automatic preparation is enabled by default, and the user can turn it off before completing setup or later. The reviewer is chosen independently of the default coding agent.

The first release exposes these settings:

| Setting | Initial behavior |
| --- | --- |
| Default coding agent | User selects an available Claude Code or Codex CLI configuration; an individual session can use either configured tool. |
| Review agent | User selects a configured tool independently of the coding-agent default. |
| Automatic review preparation | Enabled by default, explained during setup, and switchable on or off. |
| External editor | Vim by default; configurable for diff inspection and comment editing. |
| Notifications | Persistent in-app indicators and configurable macOS notifications, subject to OS permission. |
| Dashboard exit | Quit and stop local activity by default; explicit detach is also available. The default exit behavior is configurable. |
| Task base branch | Repository default branch by default, with an override when starting a task. |

Squash is the default PR merge method. An explicit auto-merge choice belongs to the individual PR and starts off.

Settings must describe working capabilities. Future options are not exposed as nonfunctional controls.

## Projects, tasks, worktrees, and sessions

### User-visible model

| Concept | Meaning |
| --- | --- |
| Project | A registered local Git repository and its worktrees and tasks. |
| Task | An identifiable stream of coding work, associated with a project and worktree, with session history and an optional linked PR. |
| Worktree | The checkout and branch in which the task's agent works. |
| Agent session | A conversation and interactive runtime using Claude Code or Codex CLI, attached to a task's worktree. |

Users add existing local repositories. The dashboard discovers their existing worktrees and allows explicit adoption of an existing worktree to continue work. Discovery alone never starts an agent or changes repository files. Cloning repositories remains available through ordinary Git tooling rather than a dedicated dashboard flow.

A new independent coding task gets a new branch and worktree based on the selected base branch. The project, task identity, branch, worktree path, and agent must be easy to identify without inspecting terminal output. The precise task-creation flow is left to UI design.

The dashboard permits one active coding-agent session per worktree in this release. Historical sessions remain accessible, and a subsequent session can use either supported tool. Switching tools starts a new native conversation; it does not transfer the previous tool's conversation state.

### Lifecycle and history

Starting, interrupting, stopping, resuming, archiving, removing a worktree, and deleting history are distinct operations.

- Exiting an agent through its native CLI stops that session's process without deleting its history or task context.
- Resume uses the agent's supported native session recovery. Retaining readable history is guaranteed separately from the ability to resume a native conversation.
- If native resume is unavailable, explain why and allow a new session in the retained worktree without pretending the old conversation resumed.
- Archiving is explicit. Agent exit, turn completion, or PR merge does not automatically mean the task should be archived.
- Archiving hides a task from active work while retaining its history and checkout.
- Worktree cleanup is a separate action with checks for uncommitted and unpushed work. Cleanup must not silently discard that work.
- Removing a worktree does not remove session history or review records. The UI explains when a removed checkout prevents resuming work in place.
- History remains until explicitly deleted. Archived tasks and stopped sessions remain discoverable through navigation and search.

The first release guarantees history capture and recovery for sessions launched through the dashboard. Importing sessions previously started outside the product is deferred.

### Readable state

The dashboard distinguishes active work, questions or approvals needing input, completed turns, stopped sessions, failures, and interruptions. Task archival is separate from agent state.

State includes enough context to understand what needs attention. A quiet terminal alone must not be treated as proof of completion or a request for input. Uncertain or unavailable state must be identified rather than reported as a known outcome. A failing test is useful context but does not by itself mean the agent session has failed if the agent is still working on it.

## Keyboard interaction and diff inspection

Both core workflows are fully keyboard-operable. Dashboard navigation provides Vim-style movement, arrow navigation, search, and discoverable keyboard help. Exact bindings and responsive layout details are UI-design decisions within the dashboard image's target composition.

Dashboard navigation and native agent interaction have explicit focus. Attaching to a session gives the native CLI control of input, including its prompts, approvals, and normal shortcuts. A documented escape shortcut returns to dashboard navigation. For example, Ctrl-C within an attached session retains the agent's native interrupt behavior; it is not redefined as a global quit action.

The user can move between projects, tasks, sessions, and reviews while background work continues. Opening an external editor and returning restores the previous dashboard context.

Diffs follow the user's current job:

| Context | Default comparison |
| --- | --- |
| Coding task | Current uncommitted changes, including staged, unstaged, and new files, with their state distinguishable. |
| Whole-task inspection | An available alternative showing the task's branch changes relative to its base, including current working changes. |
| PR review | The PR's changes at the identified reviewed revision. |

The coding-task default is always working changes, rather than remembering a different comparison from a previous inspection. The user can explicitly select the whole-task comparison.

Diffs can be inspected in Vim by default or a configured alternative editor. PR revision context must remain clear; a review diff must not silently show unrelated local working changes. Dedicated staging, commit, and push controls are deferred.

## Giving reviews

### Discovery and background preparation

The review inbox discovers personal GitHub review requests across repositories accessible through the authenticated account. It does not require those repositories to be registered local projects. Team-only review requests are outside the initial discovery scope.

When automatic preparation is enabled during setup, all outstanding personal review requests enter the preparation queue. Newly discovered requests also enter it. Manual preparation remains available when automation is disabled.

Prepare one PR at a time initially. Queued, preparing, ready, stale, failed, and submitted states are visible. Users can pause the queue or cancel preparation. Requests that are closed, merged, or withdrawn leave the preparation queue; saved drafts remain accessible.

The review agent examines the PR description, diff, relevant repository files, and reported CI results. It obtains repository context as needed without requiring a registered local project. It does not automatically run builds, lints, or tests in this release. Missing, pending, or unavailable CI evidence must not be represented as a passing result.

Background preparation must leave dashboard navigation and coding sessions responsive. A ready draft can contain no suggested issues; that is distinguishable from an incomplete or failed preparation.

### Local draft and human judgment

Open the draft beside the corresponding diff. Each suggestion has enough location and context to evaluate it. The user can:

- Explicitly keep a suggestion, including an explicit keep-all action.
- Edit a suggestion inline or in the configured external editor.
- Discard a suggestion.
- Add their own inline comments and an overall review message.

Suggestions begin undecided. Only explicitly kept suggestions and the user's own selected comments enter the submission preview. Saving an external edit returns the text to the local draft. Edits and choices are saved locally across navigation, editor use, and restart; editing or discarding a suggestion does not publish anything.

The user chooses Comment, Approve, or Request changes. The agent never chooses or submits that outcome on the user's behalf. A final preview identifies the PR, revision, outcome, overall message, and exact comments to be published. Publishing requires an explicit submission action.

### Revisions, stale drafts, and revalidation

Every draft identifies the revision examined by the agent. New commits mark the draft stale and make the difference from the latest revision visible. Existing suggestions and human edits remain attached to their original context until explicitly reconciled.

Regeneration is explicit, including when automatic preparation is enabled. It preserves the earlier draft and human edits rather than silently overwriting them or marking new suggestions as kept.

A stale draft cannot be submitted merely by dismissing a warning. The user must inspect the latest changes and explicitly revalidate the selected comments against the current revision. Regeneration is optional. Comments whose locations are no longer valid must be relocated or discarded before inclusion.

The record distinguishes the revision the agent examined from the later revision the user validated. Submission refreshes the PR revision and eligibility; if the revision has changed again, revalidation is required before publication.

### Completed reviews and renewed requests

A successfully submitted review leaves the active inbox. Its history retains the reviewed revision, outcome, and submitted comments.

Completed reviews are not actively monitored for further commits and do not generate ongoing change notifications or automatic drafts. When the user reopens one, refresh its state and show whether it changed since the recorded review.

A renewed review request returns the PR to the inbox and creates a new review cycle, even if its revision has not changed. Previously submitted comments do not automatically become pending suggestions again. Manual preparation remains available when revisiting the PR.

## Feedback and merging on the user's own PRs

The dashboard associates a task with its PR, including PRs opened through the native agent or ordinary CLI commands. It shows the linked PR's status and new reviews and comments so the user can continue the task without repeatedly switching to GitHub.

The user reads feedback in the dashboard and explicitly sends selected feedback to the task's agent. Receiving feedback alone never starts code changes. The user can then inspect the resulting working changes through the normal task-diff workflow.

Committing, pushing, and creating PRs remain actions performed by the user or native agent CLI. Replying to GitHub conversations and resolving threads remain outside the first release.

Merging a task-linked PR is available as an explicit dashboard action. The confirmation identifies the PR, target branch, current revision, and merge method. Squash is the default, and GitHub permissions and repository rules govern eligibility. Blocked merges show the reason rather than bypassing those rules.

Where GitHub supports it, the user can explicitly enable auto-merge for an individual PR. It starts off, shows its pending state, and can be cancelled. Enabling it makes clear that the merge may happen later, after additional commits, subject to GitHub's rules. Completion or failure is reflected in the linked PR state. Merging does not automatically delete the worktree or task history.

## Notifications, persistence, and interruptions

### Quit and detach

Ordinary quit follows the local-app expectation: stop the product's coding agents and review activity while retaining history, drafts, queued work, and working changes. The exit default is configurable, and the action's meaning must remain clear.

| Action or interruption | Required behavior |
| --- | --- |
| Exit a native agent | Stop that session's process; retain history, task, and checkout. |
| Detach from the dashboard | Keep agents, review discovery, and preparation running while the Mac is awake and the user is logged in. |
| Ordinary quit with the default setting | Stop local agents and review activity; preserve recoverable state. |
| Unexpected dashboard or terminal closure | Running work survives loss of the dashboard; reopening reconnects without starting duplicate sessions. |
| Sleep and wake | Do not promise progress during sleep. Refresh remote state and catch up after waking without duplicate preparation. |
| Reboot or failure of execution itself | Restore saved records, show interrupted work, and offer explicit coding-session recovery. Do not silently restart coding. |

### Attention and failure recovery

Persistent in-app indicators identify input or approval requests, completed turns, failures, ready or stale drafts, and new feedback on task-linked PRs. Configurable macOS notifications identify the relevant task or PR and provide a way back to it. Opening a notification must not submit a review, send feedback to an agent, or merge a PR. Ordinary output does not generate notifications.

Local history and draft editing remain available offline. Remote information shows when it is unavailable or no longer current. Network or authentication failures must not erase local work.

Temporary background-review failures receive a bounded number of retries with visible progress. Persistent failures show an actionable explanation and manual retry. Interrupted or partial drafts never appear ready, and retries or regeneration preserve human edits.

Failed submissions preserve the draft. If publication may have succeeded before the connection failed, reconcile with GitHub before allowing a retry that could duplicate comments or a review. Do not label an uncertain result as either confirmed failure or confirmed success. Merge and auto-merge actions likewise show their confirmed or uncertain remote state.

## Release acceptance criteria

The first usable release must satisfy both core workflows with Claude Code and Codex CLI. Automated scenarios should cover repeatable behavior, while real macOS integration checks establish that native tools, editor interaction, notifications, and recovery actually work.

### Coding coordination and own-PR continuation

| ID | Scenario and passing result |
| --- | --- |
| C1 | Add existing local repositories. Their worktrees appear; discovery changes no files and starts no agents. Explicit adoption makes existing work available. |
| C2 | Start independent tasks with each supported agent. Each new task has a separate branch and worktree, and the dashboard prevents a second active managed coding session in the same worktree. |
| C3 | Navigate projects, locate a task by search, attach, answer a native prompt, and return to dashboard navigation entirely by keyboard. Native input and interrupt behavior remain usable. |
| C4 | Create committed, staged, unstaged, and new-file changes. The default task diff shows working changes with distinguishable state; the whole-task comparison also includes earlier branch commits. Open both in Vim and return to the same context. Verify a configured alternative editor can be invoked. |
| C5 | Exit an agent, reopen the dashboard, and find its history and worktree. Supported native resume reconnects the original conversation; unavailable resume is explained without losing readable history. |
| C6 | Exercise detach, ordinary quit, unexpected dashboard closure, and reboot. Results match the interruption table, preserve working changes and history, and do not create duplicate or silently restarted coding sessions. |
| C7 | Archive a task and separately remove an eligible worktree. History remains accessible. Uncommitted or unpushed work triggers cleanup checks rather than silent loss. |
| C8 | Link a task PR, receive feedback, and read it in the dashboard. No coding starts until selected feedback is explicitly sent to the agent. Inspect resulting changes. |
| C9 | Explicitly squash-merge an eligible PR. Verify blocked merges show GitHub's reason. Where supported, enable and cancel auto-merge and observe a later completed merge without deleting task history. |

### Review discovery, preparation, and submission

| ID | Scenario and passing result |
| --- | --- |
| R1 | Complete setup using existing CLI authentication, choose a reviewer, and retain automatic preparation's enabled default. Outstanding personal requests enter the queue, including one for a repository without a registered local project. |
| R2 | Prepare queued requests serially while coding sessions produce output. Pause and cancel controls work. Disabling automation stops new automatic preparation while preserving manual preparation and existing drafts. |
| R3 | Prepare a review with description, diff, relevant repository context, and reported CI results. No build, lint, or test command is automatically run. The ready draft identifies its examined revision; missing CI and a completed draft with no issues are represented accurately. |
| R4 | Open a draft beside its diff, keep one suggestion, discard another, edit a comment in Vim, and add an original comment. Reopen after restart and verify saved text and choices. The preview contains exactly the selected content. |
| R5 | Explicitly submit each supported outcome: Comment, Approve, and Request changes. Neither draft generation nor editing publishes anything. Undecided and discarded suggestions are absent from the submitted review. |
| R6 | Push commits after draft preparation. The draft becomes stale and cannot be submitted through warning dismissal alone. Inspect the latest changes, revalidate selected comments, and relocate or discard invalid anchors before submission. A further revision change requires revalidation again. |
| R7 | Regenerate a stale draft. Earlier drafts and human edits remain available; replacement suggestions are undecided. Record the agent-examined revision separately from a later human-validated revision. |
| R8 | Submit a review. It leaves the active inbox. Later commits cause no ongoing notification or automatic preparation; reopening refreshes its changed-since-review state. A renewed request at either the same or a new revision returns it to the inbox without re-queuing previously submitted comments. |
| R9 | Close, merge, or withdraw a queued request. It leaves the queue while saved drafts remain accessible. |
| R10 | Interrupt preparation or simulate temporary, persistent, offline, and authentication failures. Observe bounded retries, actionable failure states, saved edits, manual recovery, and no incomplete draft labelled ready. |
| R11 | Simulate publication succeeding remotely while its response is lost. Reconcile before retrying and verify that the final GitHub review and comments are not duplicated. |

### Responsiveness and automation

The reference workload is three registered projects, six concurrent coding sessions, and ten tracked PRs, with agent output streaming and one review being prepared.

- Dashboard navigation responds within 100 ms under that workload.
- Attaching to an already-running local session takes under one second.
- While background activity is running, the Mac is awake, and GitHub is reachable under normal service conditions, new requests, task-linked feedback, and active-draft revision changes are normally discovered within two minutes.
- Failures or stale remote data remain visible when service or authentication conditions prevent that discovery target.

Automate scripted workflow scenarios, simulated agent output, GitHub event sequences, interruption and failure cases, and responsiveness measurements. Complement those checks with real Claude Code, Codex CLI, GitHub, and editor checks on macOS. Simulations alone do not prove native compatibility. Detailed fixture sizes, timing methodology, and test harness design belong in the validation and engineering plan.

## Explicit non-goals and future possibilities

These are outside the first release:

- Windows, Linux, remote execution, cloud workspaces, and multi-device synchronization.
- A product account, hosted session history, team administration, or a shared review inbox.
- Arbitrary agent-command support beyond the two supported integrations, or a replacement conversation interface that hides their native CLIs.
- Importing historical sessions started outside the dashboard or transferring native conversation state between agent tools.
- Multiple active coding agents sharing a worktree. Keep this in mind as a future configurable capability without exposing it as a launch setting.
- Automatically running review builds, lints, or tests. Future configuration may enable selected commands; it is deferred rather than merely switched off in this release.
- Dedicated cloning, staging, commit, push, or PR-creation controls.
- Replying to GitHub conversations, resolving threads, or automatically acting on feedback.
- Automatic regeneration of stale drafts or ongoing monitoring of completed reviews without a renewed request.
- Automatic publication of agent-generated reviews or automatic merging without a user's explicit merge or auto-merge decision.

## Unresolved details and subsequent documents

The product name and license remain undecided. Neither is inferred from Conductor or the visual reference.

UI design will refine the target composition for terminal sizes, exact keybindings, task-creation interactions, comparison controls, and presentation of history and revision reconciliation while preserving the behavior defined here. Compare usable implementation slices against the dashboard image; report material deviations and feasibility issues with their consequences and proposed alternatives.

Engineering must validate supported native-tool versions and their status, transcript, approval, interruption, and resume behavior. Compatibility limitations must be explicit; the release cannot claim support based solely on simulated interactions. Architecture, persistence mechanisms, process management, GitHub integration details, and the automated validation harness belong in separate documents.
