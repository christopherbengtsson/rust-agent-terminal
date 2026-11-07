# Milestone 3 — GitHub review preparation and human submission

Status: ready-for-agent
Resolved: no

## Problem Statement

The developer can coordinate coding work across local projects and workspaces,
but incoming GitHub review requests still require switching tools to discover
requests, inspect changes and CI, collect findings, and publish a review. Agent
help must remain a starting point for human judgment: a plausible draft is not
permission to publish, and an old draft must not silently describe newer code.

Milestone 2 supplies managed coding execution, retained history, persistence,
comparisons and external-editor handoff. It does not supply GitHub discovery,
restricted review execution, structured review drafts or publication recovery.
Ordinary coding sessions cannot simply be repurposed as background reviewers.

## Solution

Complete PRODUCT R1–R11 on local macOS. Discover personal GitHub review requests
through existing GitHub CLI authentication, including repositories without a
registered local project. Prepare one PR at a time using a separately configured
Claude Code or Codex reviewer with enforced inspection restrictions. Present a
local editable draft beside its identified comparison. Preserve human choices,
revalidate changed comparisons, and publish only the exact content and outcome
the user explicitly previews and submits.

Use Conductor's documented diff, comment and review interactions where applicable,
adapted to keyboard navigation and the existing dashboard composition. Its
documented workspace review workflow is a reference, not evidence that it
implements this incoming-review workflow. Exact styling remains a dedicated UI
design concern; usable implementation slices must still provide working controls
and follow the established visual direction.

## User Stories

1. As a developer, I want to reuse GitHub CLI authentication, so that reviewing requires no separate product account.
2. As a developer, I want actionable guidance for missing tools or authentication, so that I can recover without guessing what failed.
3. As a developer, I want to choose the reviewer independently of my coding-agent default, so that the tools can serve different purposes.
4. As a developer, I want setup to explain agent usage costs and automatic preparation, so that I understand background activity before enabling the workflow.
5. As a developer, I want automatic preparation enabled by default with a setup opt-out and later setting, so that automation remains an informed choice.
6. As a developer, I want personal review requests discovered across accessible repositories, so that local project registration does not hide work needing me.
7. As a developer, I want outstanding and newly discovered eligible requests queued automatically when enabled, so that I do not have to prepare each one manually.
8. As a developer, I want manual preparation when automation is disabled, so that I can control agent usage directly.
9. As a developer, I want queued, preparing, ready, stale, failed, incomplete and submitted results distinguishable, so that I know what is actionable.
10. As a developer, I want only one PR prepared at a time while coding continues, so that review automation does not serialize my coding sessions.
11. As a developer, I want Pause queue to stop new preparations while the current attempt may finish, so that I can control future work independently of current work.
12. As a developer, I want Cancel preparation to stop the current attempt, so that cancellation does not merely hide ongoing usage.
13. As a developer, I want disabling automation to prevent queued automatic work from starting while preserving requests, drafts and manual preparation, so that switching it off loses no work.
14. As a developer, I want closed, merged and withdrawn requests removed from preparation eligibility while drafts remain accessible, so that obsolete requests do not consume agent usage.
15. As a developer, I want preparation to inspect the description, pinned diff, relevant files and reported CI evidence, so that findings have enough context.
16. As a developer, I want unavailable or pending CI represented honestly, so that absence of evidence is never presented as passing checks.
17. As a developer, I want background review prevented from running arbitrary commands, modifying coding checkouts or writing to GitHub, so that preparation has a smaller authority than coding.
18. As a developer, I want repository-specific review instructions and skills used first, so that review respects the repository's own practices.
19. As a developer, I want general repository conventions respected even when a fallback review skill is used, so that fallback does not erase local standards.
20. As a developer, I want a bundled review skill when repository review guidance is absent, so that review works without installing a skill collection.
21. As a developer, I want an optional custom fallback during setup and afterward, so that I can change the default review approach.
22. As a developer, I want guidance taken from the pinned target branch and proposed guidance changes shown as changes, so that a PR does not silently change the rules for its own review.
23. As a developer, I want the instructions and skill versions used recorded with the draft, so that I can understand what guided its findings.
24. As a developer, I want unusable repository guidance explained with Retry or explicit Use fallback, so that substitution is never silently presented as following repository rules.
25. As a developer, I want required execution steps that the restricted reviewer cannot perform reported as unverified, so that the review makes no unsupported verification claims.
26. As a developer, I want large PRs inspected in manageable pieces where feasible, so that an arbitrary product size cap does not prevent review.
27. As a developer, I want incomplete inspection to preserve findings and explain coverage and limits, so that partial work is useful without appearing complete.
28. As a developer, I want a completed review with no findings distinguished from failed or incomplete preparation, so that silence has an understandable meaning.
29. As a developer, I want a draft beside its matching revision-specific diff with sufficient comment context, so that I can judge each suggestion.
30. As a developer, I want suggestions initially undecided and explicit Keep, Keep all and Discard actions, so that preparation does not select comments on my behalf.
31. As a developer, I want to edit suggestions inline or in my configured editor and add original inline comments and an overall message, so that the published review expresses my judgment.
32. As a developer, I want exact edited text and choices saved across navigation, editor return and restart, so that reviewing does not lose work.
33. As a developer, I want Comment, Approve and Request changes chosen by me, so that an agent never chooses my review outcome.
34. As a developer, I want an exact preview naming the PR, comparison, outcome, overall message and selected comments, so that I can inspect what submission will send.
35. As a developer, I want undecided and discarded suggestions excluded from publication, so that generating or editing suggestions cannot publish them accidentally.
36. As a developer, I want both head and base/comparison revisions retained, so that author commits and target-branch changes are handled explicitly.
37. As a developer, I want stale drafts to retain original context and edits while showing current changes, so that updated code does not silently relocate my review.
38. As a developer, I want selected comments explicitly revalidated against the current comparison and invalid anchors relocated or discarded, so that warning dismissal cannot substitute for inspection.
39. As a developer, I want regeneration explicit and earlier drafts preserved with replacement suggestions undecided, so that automation cannot overwrite human work.
40. As a developer, I want agent-examined and later human-validated comparisons distinguished, so that the record explains who evaluated which code.
41. As a developer, I want eligibility and comparison refreshed before publication, so that another change after preview blocks outdated submission.
42. As a developer, I want uncertain publication reconciled before any duplicate-capable retry, so that a lost response does not duplicate reviews or comments.
43. As a developer, I want confirmed submitted reviews removed from the active inbox with their history retained, so that completed work stays accessible without ongoing monitoring.
44. As a developer, I want reopening completed work to refresh changed-since-review information, so that I can revisit it without automatic regeneration.
45. As a developer, I want a renewed personal request to create a new review cycle even at the same revision, so that a fresh request is not confused with previously completed work.
46. As a developer, I want temporary preparation failures retried visibly within a bounded budget and persistent failures left recoverable, so that automation neither gives up silently nor retries forever.
47. As a developer, I want detach and dashboard loss to preserve background activity while ordinary quit stops it, so that review activity follows the app's lifecycle controls.
48. As a developer, I want reopening after quit or reboot to resume discovery and eligible preparation under the saved setting, so that automatic review can recover without automatically restarting coding or publishing anything.
49. As a developer, I want navigation, attach, Stop and saved coding history to remain usable during review activity and storage trouble, so that the new workflow preserves coding coordination.
50. As a developer, I want guidance-setting changes to affect the next explicit preparation while old drafts retain their provenance, so that changing a setting neither consumes usage nor rewrites saved work.
51. As a developer, I want edited text preserved after editor or save failures, so that a failed return does not lose my review comments.
52. As a developer, I want returning edits checked against their original draft and text version, so that editing cannot silently overwrite newer work or validate changed code.

## Implementation Decisions

### Ownership and integration

- Keep one stable-Rust package, the user-wide supervisor, one controlling
  dashboard and the sole SQLite writer. Blocking Git, process, filesystem,
  persistence and network operations remain in bounded cancellable workers.
- Add concrete ownership for review discovery, serial preparation, structured
  drafts and publication. Reuse supervisor command/event routing and attributed
  completion patterns; architecture refinement determines exact module ownership.
  Review commands target stable identities independently of current UI selection.
- Review identity and execution do not require a coding workspace or registered
  project. Serial preparation admission, coding-worktree admission and persistence
  scheduling are independent. Never grant reviewers arbitrary workspace setup or
  normal coding-launch authority merely to reuse process mechanisms.
- Preserve existing coding lifecycle, canonical worktree exclusivity, terminal
  restoration, durable capture, native resume and recovery guarantees. Restrict
  prerequisite refactors to changes justified by this milestone's behavior.

### Selected architecture refinement: external-editor workflow

- Deepen the existing external-editor module alongside review-comment editing,
  rather than creating a separate broad refactor milestone. Concentrate artifact
  preparation, owned editor execution, readback-before-cleanup, completion and
  recoverable failures behind one workflow interface. The implementation may
  have internal worker seams; callers should not coordinate separate preparation
  and running receivers or infer outcomes from an inspection boolean.
- Keep terminal release/reacquisition and navigation context in the dashboard.
  The editor module signals readiness for handoff before executing the editor,
  and completion before dashboard restoration. Blocking preparation, process
  waits, text readback and cleanup remain off the UI loop. Preserve the existing
  independent process group and terminal restoration behavior, including launch
  failure and disconnected-worker paths. Do not move Ratatui rendering into the
  editor module or treat dashboard loss as permission to affect coding agents.
- Keep draft identity, text-version checks, selection, revalidation and durable
  saving in the review draft module. Editor completion supplies an attributed
  result; it is not a durable-save acknowledgement. The workflow retains recovery
  material until the matching save is confirmed or the user explicitly discards
  it. Recovery material must remain identifiable after restart; staging text does
  not itself claim a committed draft edit.
- Freeze the review cycle, draft generation, comment or overall-message target,
  text version and original text at edit start. Successful changed text is saved
  locally to that target; unchanged text is a no-op. Editing preserves selection
  and validation choices and never selects an outcome or publishes. A newer text
  version or superseded draft prevents silent overwrite and offers explicit
  reconciliation with the retained edit. A PR comparison change alone allows the
  original-target text save but retains staleness and required revalidation.
- An unsuccessful editor exit, unreadable/missing edit artifact or failed durable
  save leaves the original committed draft intact and preserves available edited
  text for recovery. Explain the failure and offer applicable retry, reopen,
  apply-recovered-text or discard actions. Recovered text requires explicit apply
  after unsuccessful execution; retries retain the same intended target and do
  not create duplicate comments. Confirmed saved text may replace the original
  only after the matching writer acknowledgement, including reconciliation after
  lost acknowledgements.
- Preserve existing read-only comparison inspection and explicit live-source
  editing semantics; their outcomes do not become review text saves. Use these
  real editor uses and configured executables to guide a small concrete module,
  not a generic editor plugin system. The deletion test is whether removing the
  workflow spreads artifact lifetime, process completion and recovery ordering
  back across callers; a forwarding module alone earns no depth.
- Defer extracting comparison presentation until the pinned-review producer
  demonstrates shared behavior beyond passing captured text and a position.
  Keep Working/Whole workspace capture intact and pinned review comparisons
  distinct. Any eventual extraction must concentrate actual shared presentation,
  identity or export rules while preserving each producer's completeness rules.
- Retain the existing deep agent-session and concrete operation-owner modules.
  Reuse their ownership and attributed-save patterns rather than generalizing
  coding-specific records into a generic operation engine or coding/review runner.
  Restricted-reviewer qualification remains an earlier gate for dependent review
  features; the editor refinement does not establish provider feasibility.

### Setup, discovery and queue

- Use existing GitHub CLI authentication and supported native agent credentials.
  Expose reviewer selection independently of coding defaults. Explain costs,
  automatic preparation's enabled default and its opt-out before setup completes.
- Discover personal requests, excluding team-only requests, across accessible
  repositories. Maintain explicit remote freshness and actionable offline/auth
  failures. Under normal reachable-service conditions, target discovery of new
  requests and active-draft revision changes within two minutes.
- Queue outstanding and new eligible requests when automation is enabled;
  permit explicit manual preparation when disabled. Prepare one PR at a time.
  Pause blocks new starts; Cancel stops the current preparation. Disabling
  automation blocks queued automatic starts without deleting work or cancelling
  the current attempt implicitly. Re-enabling admits outstanding eligible work.
- Closed, merged and withdrawn requests leave preparation eligibility while
  retained drafts stay accessible. A renewed request creates a new review cycle
  even at an unchanged comparison, without reviving submitted comments.
- Detach and dashboard loss preserve owned review activity. Ordinary quit stops
  it and retains recoverable state. On reopening after quit or reboot, discovery
  and eligible automatic preparation may resume under the saved setting.
  Interrupted attempts stay visible; refresh eligibility and revisions before a
  fresh attempt. Cancellation requires explicit restart. Publication is never
  automatically repeated under preparation recovery rules.

### Restricted inspection and guidance

- Prove the accepted restricted-review contract with both supported providers
  early, before building the complete workflow on an assumed capability. Enforce
  prevention of arbitrary shell execution, coding-checkout mutation and GitHub
  writes while retaining existing native authentication. Prompt wording alone
  does not establish enforcement. An incompatible provider returns as an explicit
  decision, not a silent unrestricted fallback or a claim of full support.
- The product obtains revision-pinned description, diff, relevant repository
  context and reported CI evidence. Reviewer output is validated as structured
  draft content. Agent logs or terminal transcript parsing are not the authority
  for draft text, selection or successful completion.
- Follow applicable repository-specific review instructions and review skills
  first, taken from the pinned target-branch revision. General repository
  conventions apply whether repository guidance or a fallback skill is used.
  Instructions added or changed by the PR are proposed changes, not silently
  authoritative instructions for their own review.
- Bundle only review-preparation skills, using Matt Pocock's code-review approach
  as the starting point. Adapt for unattended restricted inspection: assess
  repository standards and originating requirements when available, report absent
  evidence honestly, and produce findings without an interactive interview or
  publishing effects. Do not assume its current parallel-agent procedure works
  unchanged in both integrations; execution strategy needs qualification.
- Use the bundled review skill when repository review guidance is absent. Setup
  explains this precedence and offers an optional custom fallback; it remains
  configurable afterward. Record guidance provenance and skill versions with each
  preparation. Existing drafts retain original guidance, while changed settings
  take effect at the next explicit preparation or regeneration.
- Unusable existing guidance causes an actionable failure with Retry or explicit
  Use fallback. Do not silently substitute. Readable guidance requiring forbidden
  execution retains the instruction and reports that step unverified. No review
  build, lint or test command runs automatically.
- No arbitrary product cap on PR size, file count or review duration is agreed.
  Inspect large changes in pieces where feasible. Engineering must establish
  bounded resource use and supported workloads. Provider context, account usage
  or operational limits that prevent completion produce an incomplete result
  identifying coverage and reason, retaining findings and requiring explicit
  action to try again. A proposed product restriction requires a new decision.

### Drafts, comparisons and editing

- Persist review cycles, preparation attempts, guidance provenance, identified
  comparisons, findings, human text, selections and publication outcomes as
  structured durable records through the sole writer. Extend transactional
  migration and operation-attributed acknowledgements; do not reconstruct drafts
  from searchable history or acknowledge save before matching durable completion.
- Preserve original examined head and base/comparison identity separately from
  later human validation and publication. Establish precise GitHub comparison
  semantics and test base-only changes, including changes without a new head.
  Coding comparisons with live working changes are not review comparisons.
- Present drafts beside their matching pinned diff and anchor/context metadata.
  Suggestions begin undecided; provide explicit Keep, Keep all and Discard.
  Include only explicitly kept suggestions and the user's selected comments.
  Editing alone never implies publication or review outcome selection.
- Support inline editing, original inline comments, overall message and configured
  external-editor editing. Reuse terminal handoff and return mechanisms while
  adding exact text readback and durable draft-save outcomes. Restore dashboard
  selection/context, surface save failures and preserve text for recovery.
- A complete no-findings draft is ready; partial, interrupted, malformed or
  failed preparation is not. Regeneration is explicit and preserves earlier
  versions and human edits; replacement suggestions begin undecided.
- Changed comparisons mark drafts stale, retain original anchors and edits, and
  expose the difference from current code. Require inspection and explicit
  revalidation of selected comments; relocate or discard invalid anchors.
  Dismissing a warning is insufficient. Regeneration remains optional.

### Publication and recovery

- Only the user chooses Comment, Approve or Request changes. Preview identifies
  the PR, exact comparison, outcome, overall text and exact selected comments.
  Submission requires an explicit user action after that preview. Preparation,
  editing, navigation, settings and agent decisions cannot authorize publication.
- Refresh eligibility and comparison before the GitHub write. Changed comparison
  requires revalidation and a current preview. Bind publication to the previewed
  content; intervening edits must not change a submitted payload silently.
- Persist publication intent and enough remote evidence to reconcile a write
  whose response is lost. Confirm success only from evidence, preserve drafts on
  failure and keep uncertainty explicit. Do not permit a duplicate-capable retry
  until reconciliation establishes the prior result. Exact GitHub mechanisms and
  race handling require proof, including pending reviews and partial failures.
- Confirmed submission removes that cycle from the active inbox and retains its
  reviewed comparison, outcome and published comments. Do not actively monitor
  completed reviews for commits or automatically regenerate them. Reopening
  refreshes current state; renewed personal requests remain discoverable.
- Temporary preparation failures receive at most two automatic retries with
  increasing delays, visible count and next retry time. Refresh eligibility and
  comparisons before each attempt and retain prior partial results/human edits.
  Authentication, unusable guidance and unsupported restrictions require user
  action. Manual retry remains available after exhaustion. These rules never
  authorize automatic publication retries.

## Testing Decisions

- The user confirmed the supervisor command/event interface as the primary
  workflow test seam: discover, queue, prepare, edit, revalidate, preview, submit
  and recover. Prefer observable state, effects and durable outcomes over helper
  tests that mirror implementation. Refine ownership in the architecture pass
  without scattering separate test-only interfaces across the feature.
- Use controlled GitHub and reviewer responses for deterministic sequences,
  including renewed requests at unchanged revisions, eligibility withdrawal,
  stale replies, cancellation, retry exhaustion, missing CI, no findings,
  malformed/incomplete output and network/authentication failures. Exercise the
  integrated workflow, not only parsers in isolation.
- Use real temporary Git repositories for pinned snapshots, base/head and merge
  comparisons, renames, deleted/relocated anchors, unrelated local changes and
  target-branch-only changes. Assert that inspection does not mutate coding
  checkouts and that displayed comparisons identify the reviewed code.
- Use existing attributed-operation and persistence integration patterns for
  migration, exact draft text/choices, save failures, stale acknowledgements,
  restarts and fair writer scheduling alongside capture/search. Test durable
  structured saves independently of transcript/index availability.
- Use existing editor integration and PTY ownership patterns for exact comment
  round-trip, save/cancel/failure, resize, dashboard context return and continued
  supervisor/coding activity. Requalify installed macOS Vim and Pico checks when
  their affected terminal/editor behavior changes.
- Exercise the editor workflow interface with controlled executables that leave
  text and exit successfully, leave text and exit unsuccessfully, leave text
  unchanged, or remove the artifact. Verify readback before cleanup, text retained
  across failed saves and restart, stale/duplicate save completions, target-version
  conflicts and PR-only comparison changes. At the supervisor seam, assert exact
  durable text, unchanged selection and no publication effects. Retain existing
  live-source and inspection regression coverage. Tests should survive changes
  to private worker scheduling; replace redundant tests only after equivalent
  observable coverage exists.
- Prove enforced reviewer restrictions with both supported native tools using
  their existing authentication, including repository guidance that requests
  forbidden execution. Read the repository's agent cost controls before any
  real-provider development or qualification launch. Record versions, workload,
  source identity, commands, results and limitations.
- Verify R1–R11 end to end with native reviewers, real GitHub and installed editor
  checks on macOS. Real external writes during qualification require explicit
  user submission after an exact preview; this spec does not authorize publishing
  arbitrary test reviews. Include all three outcomes, unregistered repositories,
  base-only changes, regeneration and renewed same-revision requests.
- Inject lost publication responses after remote success and interruption around
  durable intent and remote writes. Establish remote review/comment identity and
  no duplicates before enabling retry. Include definite rejection, uncertainty
  and reconciliation that cannot yet resolve the result.
- Select affected supplemental checks before implementation: terminal restoration,
  both installed-editor checks, native committed-corpus readable/search replay
  when persistence/history paths change, and the six-stream coordination
  measurement when scheduling/capture/storage changes. Select the older latency
  and full-storage supplements when their affected boundaries change. Record
  omitted or unavailable checks as unmet gates or explicit unverified scope.
- Measure navigation and attach under concurrent coding and restricted review
  activity against the 100 ms and one-second targets; measure reachable-service
  discovery against two minutes. Measure bounded memory, queues, storage fairness
  and large-review coverage behavior. Milestone 2 evidence does not qualify new
  code; the full combined real-native release workload remains Milestone 5.
- Run stable locked build, formatting, Clippy with warnings denied and all-target
  tests after implementation. Report ordinary passed/failed/ignored counts
  separately from native, editor, GitHub and measurement evidence. Ignored tests
  do not count as passing, and Linux CI does not prove macOS native compatibility.

## Out of Scope

- Milestone 4 own-PR linking, feedback-to-coding-agent actions, merge and per-PR
  auto-merge controls.
- Milestone 5 macOS notifications, remaining general onboarding and full combined
  release qualification. Reviewer-specific setup required by R1 is in scope here.
- Team-only review discovery, hosted/shared inboxes, remote execution and new
  supported platforms.
- Automatic review builds, lints or tests; arbitrary command execution or coding
  checkout mutation by the reviewer.
- Automatic review outcome selection, publication, stale-draft regeneration or
  ongoing completed-review monitoring without a renewed request.
- Installing Matt Pocock's full collection into coding sessions, a general skill
  marketplace/installer, or unrelated architecture cleanup.
- Dedicated cloning, commit/push/PR-creation controls, GitHub conversation replies
  or thread resolution, and a replacement native coding-agent conversation UI.
- Exact UI styling and illustrative controls from the PR inbox concept as new
  release requirements. Functional diff/comment/preview interactions remain
  required; styling is refined separately.

## Further Notes

- PRODUCT and its R1–R11 criteria are the acceptance authority. This spec captures
  the milestone 3 interview decisions without claiming implementation exists.
  Accepted supervisor, restricted-preparation and two-sided-comparison ADRs remain
  binding. Review guidance terminology is recorded in the domain glossary.
- Agreed planning sequence: grilling with domain docs, this spec, a focused
  architecture survey asking “How can we make this change easy?”, update this
  spec with selected conclusions, then dependency-ordered tickets. Implement the
  spec in another session. Do not treat the initial spec as authorization for an
  unrelated refactor or skip the planned architecture reconciliation.
- The focused architecture survey selected external-editor workflow deepening
  alongside review-comment editing. Comparison presentation extraction is
  conditional on demonstrated reuse during pinned-review implementation, not a
  prerequisite. Existing session and concrete durable-operation ownership remain
  intact. The selected refinement and its tests are recorded above, so ticketing
  does not depend on the temporary visual report.
- Engineering proofs remain explicit: restricted-provider compatibility, exact
  GitHub comparison semantics and freshness races, request-cycle observation,
  publication reconciliation, guidance discovery/version identity and bounded
  review execution. Resolve these through early research/proof work and record
  evidence before dependent implementation assumes the answer. If a proof
  conflicts with confirmed behavior, return for a decision rather than silently
  narrowing scope or weakening restrictions.
- The local ask-matt router identifies code-review as the relevant review skill;
  its current Standards/Spec method supports the chosen starting point. That
  routing advice is not qualification of bundled or unattended native behavior.
- References consulted: [AI Hero architecture guidance](https://www.aihero.dev/skills-improve-codebase-architecture#when-to-reach-for-it),
  [Matt Pocock skills](https://github.com/mattpocock/skills), and
  [Conductor review workflow](https://www.conductor.build/docs/guides/review-and-merge).
- [Conductor's diff viewer](https://www.conductor.build/docs/reference/diff-viewer)
  documents local comments alongside changes. External-editor failure recovery
  and stale-edit handling above are this product's decisions; the public reference
  does not establish those behaviors in Conductor.
