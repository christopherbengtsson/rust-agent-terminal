# Milestone 2 — coding coordination

Status: ready-for-agent
Resolved: yes

## Problem Statement

The completed native-session proof lets a developer run one managed Claude Code
or Codex conversation in an adopted checkout, leave and return to it, and recover
retained history. It does not yet let them coordinate independent coding work
across projects. Current implementation assumptions about one runtime, one
current adoption and one capture stream obstruct that experience.

The developer needs a Conductor-like local workspace workflow: identify work,
start independent changes in separate branches and folders, find old context,
inspect and edit code, and put work away without losing it. Coordination must
remain responsive while agents, Git, history capture and setup commands run.

## Solution

Complete PRODUCT C1–C7 on local macOS using the proven native CLI/supervisor
foundation. Projects contain durable workspaces; workspaces contain successive
agent sessions. Independent workspaces run concurrently, with one active coding
session per canonical worktree in this release.

Offer prompt-led Create and start, automatic readable names, configured setup,
metadata/history search, conversation-specific native resume, dashboard and
external-editor diff inspection, explicit checkpoint-backed archive and separate
safe folder removal. Keep Conductor's local workflow as the reference while
making documented exceptions and source uncertainties explicit.

Begin with behavior-preserving prerequisites that deepen per-session ownership,
correlate durable operations and support bounded concurrent capture. Do not turn
this into a separate broad refactor milestone or replace the proven native
execution machinery.

## User Stories

1. As a developer, I want to register repositories already on my Mac, so that I can coordinate existing projects without moving them.
2. As a developer, I want linked worktrees and path aliases recognized consistently, so that one checkout is not accidentally treated as two execution locations.
3. As a developer, I want discovery and refresh to remain read-only, so that browsing starts no agents and changes no working files.
4. As a developer, I want to explicitly adopt existing work, so that discovery never silently creates a workspace or launches coding.
5. As a developer, I want projects and workspaces retained after restart, so that I can return to the same work and history.
6. As a developer, I want missing repositories and folders shown honestly, so that unavailable files do not erase my records.
7. As a developer, I want to switch projects and workspaces by keyboard, so that background work does not interrupt navigation.
8. As a developer, I want separate running sessions in independent workspaces, so that agents can work without sharing files.
9. As a developer, I want input, Stop and resize tied to the intended session, so that switching selection cannot affect another agent.
10. As a developer, I want a second active coding session in the same worktree refused, so that this release preserves checkout exclusivity.
11. As a developer, I want Create and start to prepare a new workspace and launch my selected agent from one explicit action, so that starting work feels direct.
12. As a developer, I want to create a workspace without starting coding, so that I can prepare it first.
13. As a developer, I want each independent workspace to receive its own branch and folder, so that my original checkout and other tasks remain separate.
14. As a developer, I want to supply a first prompt without inventing a title, so that I can describe the work immediately.
15. As a developer, I want readable names derived from the work and editable later, so that workspace identity is understandable without becoming tied to its title.
16. As a developer, I want stable folder and workspace identity when names or branches change, so that history and recovery stay associated correctly.
17. As a developer, I want the configured remote base fetched before creation, so that new work normally starts from current code.
18. As a developer, I want to select a different base and see the actual starting commit, so that the comparison context is explicit.
19. As a developer, I want an explicit local-code fallback after a fetch failure, so that I can choose to work offline without mistaking old code for current remote state.
20. As a developer, I want to create work in repositories without a remote, so that local-only projects are supported.
21. As a developer, I want optional project setup to run automatically in a new workspace with visible progress, so that dependencies and local resources can be prepared.
22. As a developer, I want failed setup to retain the workspace and offer Retry or Continue without setup, so that a failed command does not lose work or silently start coding.
23. As a developer, I want uncertain setup outcomes explained before retry, so that commands with external effects are not duplicated automatically.
24. As a developer, I want recoverable partial creation and launch outcomes, so that reconnecting or retrying cannot create duplicate branches, folders or agents.
25. As a developer, I want native prompts, approvals and interrupt behavior preserved with both tools, so that the dashboard does not replace their normal interaction.
26. As a developer, I want to return to dashboard focus and reattach to the same session, so that navigation leaves coding running.
27. As a developer, I want to find work by project, title, branch or path, so that I can locate a workspace without reading terminal output.
28. As a developer, I want to search retained conversation/output text across workspaces, so that old discussions remain useful.
29. As a developer, I want search results to include archived work and missing folders and open the matching passage, so that saved context does not depend on a live checkout.
30. As a developer, I want readable history independent of native recovery, so that losing a native conversation record does not erase captured context.
31. As a developer, I want to select an older or latest conversation and explicitly resume with its original tool, so that recovery targets the conversation I chose.
32. As a developer, I want unavailable resume explained with an explicit new-conversation option, so that starting again is never misrepresented as resuming.
33. As a developer, I want current status distinguished from historical attention, so that an old event is useful without pretending it proves current activity.
34. As a developer, I want working diffs to distinguish staged, unstaged and new files, so that I can inspect all uncommitted changes.
35. As a developer, I want a whole-workspace comparison including previous commits and current edits, so that committing does not hide earlier work.
36. As a developer, I want dashboard comparisons to update without losing selected context unnecessarily, so that inspecting an active workspace remains usable.
37. As a developer, I want either comparison opened in Vim or my configured editor, so that I can inspect with familiar tools.
38. As a developer, I want to open actual workspace files for editing and return to the same dashboard context, so that I can correct code myself.
39. As a developer, I want Stop and archive to confirm execution cleanup before saving and hiding work, so that archive cannot race a managed agent.
40. As a developer, I want unfinished code saved as a local archive checkpoint with a precise receipt, so that putting work away does not discard edits.
41. As a developer, I want checkpoint exclusions such as ignored files explained, so that saved does not imply a complete backup of every resource.
42. As a developer, I want app-created local branches cleaned up on archive by default while code remains recoverable, so that stale branch names do not accumulate.
43. As a developer, I want adopted existing branches protected unless I explicitly select deletion, so that importing work does not silently grant branch-removal permission.
44. As a developer, I want restoration to handle a reused branch name without overwriting it, so that returning to old work preserves unrelated changes.
45. As a developer, I want folder removal separate from archive and history deletion, so that each destructive choice is explicit.
46. As a developer, I want unsaved files and uncertain checks to block folder removal and local-only commits to require confirmation, so that cleanup does not silently lose work.
47. As a developer, I want ordinary quit, detach, dashboard loss and reboot handled across all workspaces, so that activity and recovery remain predictable without duplicate or automatic coding starts.
48. As a developer, I want navigation, Stop and storage recovery usable during heavy output or disk trouble, so that background activity cannot trap me.

## Implementation Decisions

### Architecture prerequisites and migration

- Keep one stable-Rust package, one user-wide supervisor, one controlling dashboard
  and one persistence writer. Retain PTYs, process evidence, native adapters,
  launch gates, explicit cleanup and existing durability guarantees.
- Deepen a concrete Agent session module. Its private implementation coordinates
  launch progression, owned execution and final capture; the supervisor routes
  targeted commands and observations without reconstructing phase bookkeeping.
  The target shape is incremental, not a large all-at-once relocation.
- Attribute durable successes and failures to exact session, request where
  applicable, operation identity and purpose. Unrelated metadata saves and a
  global pending-write count cannot establish a launch result's durability.
  Rejected/stale completions cannot advance another attempt or overwrite newer
  execution facts.
- A spawned child is owned immediately, before further fallible setup. Failed
  setup and cancellation retain cleanup obligations. Prepare commits before
  creation; authorization commits before release; verified exec receipt precedes
  result recording; only the matching durable result permits acknowledgement.
- Keep native controls and bounded presentation available during provisional
  execution: a screen may exist before result durability. Stop remains usable
  during storage stalls; late completions cannot restart cancelled work.
- Workspace owns shared files/branch and session membership. Enforce today's
  one-active-coding limit on canonical worktree identity, including uncertain
  reservations. Never encode one runtime per workspace in the durable schema.
  A later shared-agent policy must reuse session ownership, while whole-workspace
  mutations still account for every relevant managed execution and capture tail.
- Migrate current adoption, retained session/native IDs, request outcomes and
  history without loss or automatic execution. Never infer historical project or
  checkout identity from lossy display labels. Represent unknown legacy facts
  honestly. Detect incompatible live supervisors rather than competing for their
  state or migrating under an active older writer.
- Interleave multiple capture streams with bounded per-session accounting,
  batching and fair control progress. Preserve byte ownership, backpressure,
  accepted/written/durable distinctions and per-session final completion. History
  reads/indexing cannot monopolize capture or resurrect explicitly deleted text.
- Worktree discovery, identity and Git execution belong in execution-owned workers.
  Keep Unix path bytes, NUL-safe parsing, bounded jobs and per-repository mutation
  serialization. Hold mutation exclusion for actual Git work, not agent lifetime
  or model/network waiting. Preserve Git's own refusals after immediate rechecks.

### Registry, creation, setup and naming

- Persist registered projects, workspaces and session associations independently
  of current selection, current branch name and checkout existence. Re-adoption
  returns to the retained workspace. Refresh errors retain last successful data.
- New workspaces use an external product-managed per-project folder with stable
  generated names and collision-safe branches. Permit explicit path/base override;
  never overwrite an existing folder or branch. Track actual original base ref
  and commit separately from editable title, current branch and workspace ID.
- Offer explicit Create and start, optionally carrying the first prompt, and
  Create without starting. Persist attributed phase outcomes for checkout creation,
  setup and launch. A later failure retains earlier success and exposes recovery;
  retry/reconnect queries the original operation rather than repeating mutation.
- Normally fetch the configured repository remote/base. A failure offers a clearly
  labelled explicit local version choice, showing the commit used. Repositories
  without remotes use a selected local base; unresolved defaults require a choice.
- Establish first-prompt launch and naming against both real native tools early.
  Use supported invocation behavior and truthful fallback names. A supplied prompt
  can provide a local readable name; native names may be used only with dependable
  attribution. No extra coding turn, hidden AI provider, raw-key recording or
  screen-text guess is permission to invent a native conversation title.
- Names can change without relocating folders or losing identity. Observe branch
  changes made through ordinary Git without conflating another branch or PR with
  the workspace. Naming alone never starts coding. Create without start, restore,
  reconciliation and startup never submit a prompt or restart a native agent.
- Optional configured setup runs in the workspace as owned local execution.
  Capture bounded progress/errors and support cancellation and ordinary quit.
  After failure offer Retry or Continue without setup; explain whether continuing
  carries out a previously requested Create and start. Uncertain command outcomes
  require reconciliation or an explicit informed choice, not automatic replay.
- Expose only necessary working configuration: coding-agent choice/default,
  repository base/setup, editor invocation and branch-cleanup policy. Reuse existing
  native authentication; settings must not claim that reconnect refreshes the
  long-lived supervisor's environment or copies credentials.

### History, resume and attention

- Session lists and history/search results use stable identities, not mutable row
  offsets. Keep raw retained output independently of its readable/search projection.
  Maintain correct UTF-8/control/redraw handling across page boundaries and never
  execute captured terminal controls during reading.
- Search metadata and committed readable history across projects, including
  archived/missing workspaces. Use bounded, cancellable queries with attribution,
  result snippets and passage locations. Do not promise structured native chat
  import or lossless semantic reconstruction from terminal output.
- Resume selects the native conversation identity and original tool of the chosen
  historical conversation, producing a new managed runtime/history segment within
  the same workspace. Reading history does not change resume target implicitly.
  Resume does not rewind the Git checkout. Apply occupancy/uncertainty barriers;
  explain missing native records, unsupported versions and absent checkout.
- Current semantic confidence remains evidence-based and version-qualified.
  Preserve last attention as explicitly historical context when useful; expired
  evidence stays Unknown and must not appear as a fresh request. This milestone
  adds in-app coding coordination, not macOS notification delivery.

### Diffs and external editor

- Name comparisons Working and Whole workspace. Opening inspection starts with
  Working, regardless of a previous choice. Show staged/unstaged/new-file state,
  including files ordinary tracked diff output omits, without altering the index.
- Whole workspace uses the task branch's common ancestor with its selected local
  base, includes subsequent branch changes and current working edits, and labels
  the actual compared commits. Missing/ambiguous base or unrelated histories produce
  an explanation/base choice, not a silently unrelated comparison. Inspection
  itself does not fetch. This is the local comparison definition, not a claim of
  Conductor's undocumented exact algorithm.
- Present changed-file navigation and readable comparisons in the dashboard,
  refreshed through bounded worker observations that preserve selected context.
  Label snapshot freshness and unsupported/binary/conflict/submodule cases honestly;
  do not execute untrusted external diff programs as an implicit renderer.
- External diff inspection materializes content with comparison identity; actual
  source editing opens the workspace or selected existing file. Deletions and
  unavailable source locations remain inspectable without pretending an editable
  file exists. Exact line positioning is supported where the editor allows it.
- Pass editor executable/arguments and paths without shell interpolation. Release
  and reacquire terminal ownership for terminal editors; restore focus, selection,
  history/diff context and usable geometry after success, nonzero exit or spawn
  failure. Agents continue while inspecting/editing. Reopening inspection obtains
  a fresh comparison; an existing editor artifact does not silently change beneath
  the editor. Source files are live and shared with the active agent.

### Archive, recovery and folder removal

- Stop and archive first cancels pending managed coding/setup launch intent and
  establishes confirmed cleanup; account for every relevant runtime, uncertainty
  reservation and capture tail. Block new admission while changing shared Git
  state. Stop, history-settled and archived are distinct outcomes.
- Save tracked changes and new non-ignored files as a clearly named local Archive
  checkpoint. Preserve ignored files in the retained folder and explain exclusions.
  Do not create an empty checkpoint merely to archive clean code. Conflicts,
  unsupported nested repository state, Git identity/hooks failures and concurrent
  edits cannot produce a false complete-save receipt. Never silently bypass hooks,
  manufacture an author or force-add ignored files to make archive succeed.
- Before branch cleanup, pin the saved code through a workspace-specific Git
  reference and confirm durable metadata connecting it to the workspace. A hash
  string alone is not a recovery guarantee. Preserve original branch/base and
  native session identities independently of that reference.
- Delete local branch on archive defaults enabled for product-created branches;
  adopted existing branches require explicit selection. Never delete remote
  branches. Retain the folder, detaching the stopped checkout at its saved commit
  if needed. Recheck ref identity and other checkouts before deletion; do not
  overwrite another actor's new commits or force-delete a branch in use elsewhere.
- Report checkpoint save, archive visibility and branch-cleanup outcomes separately.
  Failed saving leaves unarchived. A later cleanup failure preserves confirmed
  recovery and explains the remaining branch or incomplete operation; retries
  reconcile prior outcomes rather than create duplicate commits.
- Unarchive restores visibility and a usable branch when the folder exists, without
  coding restart. A reused old name requires a collision-safe alternate choice;
  never move an unrelated ref. Missing folders remain unavailable with saved code
  and history accessible; do not pretend native resume in place is possible.
- Folder removal is a separate preview/confirm action for eligible linked worktrees.
  Exclude the main checkout, active/uncertain/locked targets and unsafe identities.
  Unsaved tracked/untracked/ignored files block removal. Explain local-only commits
  and require explicit acknowledgement before removing only their checkout folder;
  verify or create a durable recovery reference for the exact current HEAD before
  removal, including never-archived or detached worktrees. An older archive ref is
  insufficient if HEAD changed; retain previous archive recovery points as well.
  Retain saved-code recovery/history and any non-deleted branch. Unknown
  upstream/push evidence is uncertainty, not proof that work was pushed.
- Explicit transcript deletion does not delete saved code, workspace identity,
  native conversation IDs, durable launch outcomes or future review/PR records.
  Archive refs are not automatically aged out or cleaned because a transcript was
  deleted. Preserve the existing tombstone and filesystem-safety implementation.

### Later-milestone compatibility

- Restricted reviewers may inspect repositories without local coding workspaces;
  they must not inherit arbitrary coding/setup powers. Coding admission, serial
  review preparation and sole-writer persistence are separate constraints.
- Readable terminal text is not the future review-draft format. Preserve structured
  draft/anchor/choice records and independently pinned review head/base comparisons.
  Source editing deferral does not defer inline review-comment editing.
- Branch cleanup cannot erase later PR links or imply a merge/push. Future linking
  uses repository/PR identity rather than current branch-name equality. No remote
  transport, provider framework or speculative job engine is needed for this work.

## Testing Decisions

- Reuse the previously approved primary seam: real dashboard plus supervisor plus
  scripted PTY child in isolated state and actual temporary Git repositories.
  Existing state/event and rendering checks remain focused supporting evidence.
  The architecture exploration changes ownership behind that seam, not the user-
  approved verification approach.
- Good tests assert observable routing, process count/cwd/identity, exact native
  input, durable result attribution, retained bytes, saved Git state, recovery and
  visible errors. Avoid tests that merely restate message routing or private flags.
- Preserve the completed proof's lost-ack, launch crash-window, stalled-prepare
  cancellation, provisional receipt, descendant cleanup, full-storage/backpressure
  and explicit history cleanup scenarios. Do not weaken assertions to land a
  refactor; add stale/cross-session completion and partial-spawn failure coverage.
- Use real repositories for alias/adoption identity, creation/fetch fallback,
  collisions, branch/base movement, staged/unstaged/new/ignored changes, conflicts,
  branch cleanup, recovery-ref survival under Git GC, restoration and unsafe removal.
  Check original checkout/index, unrelated branches and excluded files are intact.
- Verify migration of retained Milestone 1 state and history, reordered/deleted
  search results, committed-prefix reads, multi-session final flushing, cancelled
  queries and storage faults while search/capture/controls run together.
- Exercise actual Vim and one configured alternative editor on macOS: both
  comparisons, real source editing, nonzero/spawn failure and terminal/resize/
  context restoration. Simulations alone do not establish editor compatibility.
- Record real Claude Code and Codex versions and demonstrate creation/initial prompt,
  readable/searchable history, native questions/approvals, Ctrl-C, resize, focus
  escape, same-session attach, selected older-conversation recovery, and scoped
  quit/reconnect/archive behavior. Earlier versioned proof is prior art, not new
  evidence for changed launch paths or untested versions.
- Measure a declared coding-only workload of three projects and six concurrent
  sessions, streaming output and history searches/diffs. Report input-to-visible
  navigation distribution/worst case against 100 ms and usable attach against
  under one second, plus per-session durability, queues/memory and storage growth.
  Retain the healthy one-second flush target and failure/backpressure distinctions.
- Exercise detach, ordinary quit, unexpected dashboard loss, supervisor failure,
  sleep/wake and actual reboot recovery with no silent restart. Distinguish real
  reboot evidence from injected boot fixtures. Arrange any physical reboot with
  the user; do not perform it without explicit permission. An unavailable required
  case remains unverified and blocks a C6 completion claim.
- At usable slices compare normal/small terminal captures with the vision image,
  preserving reviews-above-project-tree/native-session composition and keyboard
  discoverability. Record unsupported layouts and actual terminal limitations.
- Run stable locked build, formatting, warnings-denied all-target Clippy and
  all-target tests at meaningful implementation checkpoints and final integration.
  Do not call this documentation work an implementation or test pass.

## Out of Scope

- GitHub review discovery/preparation/publication, own-PR feedback/merge actions,
  remote/cloud execution and the combined reviewer/PR release workload.
- Multiple active coding agents sharing a worktree and editing source directly
  inside dashboard diffs; keep their migration seams, not placeholder controls.
- Open GitHub project/cloning and Quick start/new-repository creation. These are
  documented later additions with no delivery milestone assigned.
- Dedicated staging/commit/push/PR-creation controls, broad app-running actions,
  archive scripts, a special ignored-file-copy system, general onboarding and
  macOS notifications beyond setup needed by implemented M2 features.
- Importing externally started conversations, cross-tool native-state transfer,
  a replacement agent chat interface, arbitrary tool integrations and automatic
  code/history retention policies.

## Further Notes

- Existing working behavior and limitations are recorded in the [Milestone 1
  proof](../../docs/proofs/18-milestone-evidence.md). This spec does not reopen or
  re-label that completed scope.
- Sources: [PRODUCT](../../PRODUCT.md), [vision](../../VISION.md),
  [architecture](../../docs/architecture.md), [milestones](../../docs/milestones.md),
  [glossary](../../GLOSSARY.md), accepted ADRs 0001–0007, the
  [interview](interview.md) and [Conductor research](conductor-research.md).
- The Conductor references establish product direction, not undocumented guarantees
  about its exact offline, cleanup, diff or local setup-failure implementation.
- The user approved the ticket granularity and blocking edges. The
  [19 implementation tickets](ticket-breakdown.md) are published under issues;
  publication does not claim that implementation or validation has started.


## Integrated qualification checkpoint — 2026-11-07

Ticket 19 records new source-pinned aggregate, declared six-session fixture and
actual six-native-session evidence in the [qualification record](../../docs/proofs/m2-19-qualification.md).
The corrected frozen serial all-target aggregate passed at 335ba44; the later
a56 measurement-only refinement preserves the exact production source/executable.
Native evidence qualifies Claude 2.1.318 and Codex 0.167.0; a separate current
Claude 2.1.319 smoke retains Unknown semantics. Final integration review/checks
remain separate. Actual sleep/wake and physical reboot are still open required
gates, with an explicitly unexecuted [preparation plan](../../docs/proofs/m2-19-physical-transition-plan.md).
No physical transition is inferred from injected boot fixtures. Ticket 19 remains
Resolved: no, C6 and milestone completion are unclaimed, and no later release
gates are claimed.


## Physical qualification follow-up — 2026-11-07

The final reviewed integration gate at `59fed09` passed 278 tests with eight
intentional ignores and unchanged source/executables. The subsequent
[actual user-performed sleep/wake](../../docs/proofs/m2-19-sleep-wake.md) passed
with matching native/process identities, code/ref witnesses and committed
history prefixes. An immediate pre-reboot checkpoint was refreshed. This
supersedes the earlier sleep/wake-open status; physical reboot remains an open
required gate awaiting explicit user arrangement. Ticket 19, this spec and C6
remain unresolved/unclaimed.


## Completion — 2026-11-07

All 19 implementation tickets are resolved on the integration branch created
from `55f1e17`. Independent standards/spec findings were fixed and verified;
final stable locked build/format/Clippy and frozen all-target checks passed
(278 passed, eight intentional ignores, unchanged binaries/source at `59fed09`).
The separately user-performed [sleep/wake](../../docs/proofs/m2-19-sleep-wake.md)
and [physical reboot](../../docs/proofs/m2-19-reboot.md) checks passed. Owner-only
restore started no coding; explicit original-ID recalls and cold cleanup passed.
The [qualification record](../../docs/proofs/m2-19-qualification.md) preserves
exact source/tool/workload boundaries, failed attempts and remaining limitations.
This closes the declared M2 C1–C7 scope, not later GitHub/reviewer/notification
or combined-release gates. Earlier open-status paragraphs are historical.
