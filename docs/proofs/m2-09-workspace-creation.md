# Independent workspace creation: scoped evidence

Ticket 09 implements a usable creation flow; the upstream 06/08 integration gate
remains a separate prerequisite until its corrected full aggregate passes. This
record does not claim Milestone 2 or C6 release qualification.

## Source and environment

Feature branch `feature/m2-09-create-workspace`, isolated worktree
`/tmp/rat-m2-09`, started at integration `610c723`. Working-diff integration
`5c1a604` and the corrected compatibility implementation `c20cd49` are merged.
External-editor integration `438b75d` is also merged, preserving its protocol and
App/editor handoff behavior. The actual native smoke used source `6f4ab06` and binary SHA-256
`a33b560559fe4ccd7351841a865b82ff699a95f8df28a2a96ed650cd7b18a049`.
Subsequent amendments cover paste preservation, receipt attribution/rendering,
cancelled intent labels, relative managed paths, failed worker reporting, atomic
Git branch/operation claims and targeted cancellation after native handoff.
Focused App/Git/Controller checks cover these paths. The final short dashboard
fixture ran after the creation/Whole comparison merge and cancellation changes.

Host: macOS 26.2 (25C56), arm64; stable Rust 1.99.1, Git 2.50.0.
Native tools: Claude Code 2.1.319 and Codex CLI 0.167.0, through their existing
local authentication/configuration. Every scenario used a real temporary Git
repository and a separate explicit private supervisor state directory.

## User flow and observable boundaries

`N` opens the form for the selected registered project. Tab selects the explicit
base, suggested branch, suggested stable folder, optional first prompt and agent;
Left/Right chooses the agent. No title is required. Enter creates without starting;
Ctrl-S creates and starts. `O` opens the original creation receipt, `[`/`]`
selects an older receipt, `G` queries/reconciles without launch, and `Z` cancels
pending creation. On a failed fetch, `L` explicitly chooses the displayed local
commit and displayed launch choice. Renaming remains the existing independent
local metadata operation.

The receipt separates saved checkout facts from the originating native owner's
launch result. Provisional native results are masked until that owner's matching
durability obligations settle, just like the existing top-level launch result. It displays base and exact commit before the local fallback action.
TestBackend assertions exercise 160×40 and 60×18 forms and fallback receipts,
including an intentionally long fetch error that previously hid the commit.
The creation form/receipt occupies the native-session area while the normal
reviews-above-project-tree composition remains. Small layouts retain their
existing hidden sidebar. These are state/rendering checks; the end-to-end form
fixture below runs in a real outer PTY at 160×40, not Terminal.app/iTerm visual
qualification.

## Tests and demonstrated failures

- `creation_without_start_retains_exact_base_and_same_request_after_owner_restart`
  passes through the actual Controller/supervisor/writer seam. It proves creation
  without native launch, prompt-derived naming, exact base, original checkout
  preservation, duplicate request handling and saved receipts after owner restart.
  It also loses the dashboard around partial Git success, cancels before its late
  result settles, reconciles the same operation and retains one checkout without
  coding. An initial registry-write failure creates no branch/folder; explicit
  same-operation recovery commits the intent before finishing once. A failed
  fetch creates nothing, and explicit local fallback retains its originally
  displayed commit after the remote-tracking ref moves.
- `dashboard_create_and_start_targets_new_checkout_and_retains_failed_launch_without_duplicates`
  passes (final bounded rerun: 3.95s) through the actual dashboard, outer PTY,
  supervisor and native
  launch boundary. An isolated Claude executable records the native argv and cwd
  before replacing itself with the existing scripted PTY child. An unrelated
  workspace keeps running and selected. The supplied first prompt appears as one
  argument after `--` in the new managed folder. Duplicate Create and G preserve
  the same session/process and worktree. An intentionally absent Codex executable
  produces an attributed durable failed native-launch receipt while keeping its
  created row, folder and branch. Retry/query cannot create another checkout or
  process. Its final rerun blocks the exact originating native Prepare after
  checkout durability, cancels by creation UUID/original session UUID, releases
  storage and observes the durable cancelled outcome without native exec. An old
  receipt's cancellation leaves a later conversation in the same workspace
  Running with its unchanged session UUID. Shutdown confirms cleanup and the test
  waits for owner exit.
- Git-worker tests prove a saved base commit survives subsequent base movement;
  staged/unstaged/new original files and the original index remain unchanged.
  Same-request lost-result reconciliation retains one checkout. Ambiguous defaults
  require an explicit base, and existing branch/path collisions retain unrelated
  content. A real bare-remote/update-checkout case proves successful fetch obtains
  the newer commit without moving the original HEAD or index. Failed remote
  fetch offers the exact local commit without mutation. An operation-specific
  private Git ref atomically claims the new branch; branch-only interrupted
  creation resumes its own branch and creates exactly one checkout. A public
  coordinator/writer fault case was red before correcting reconciliation of an
  initial intent-write failure: G now always disarms the earlier requested
  launch. Cancelling that unsaved intent records only the cancelled outcome after
  repair, without creating a folder.
- App checks prove the two distinct launch choices, suggested non-UTF-8 path-byte
  preservation and unchanged pasted multiline/Unicode first prompts. Existing
  App selection/targeting tests stay green. All UI tests, including action
  discoverability and the new creation/fallback checks, stay green.

The first two real-PTY fixture attempts failed because the harness reopened before
controller release and then typed `N` into attached native focus. The corrected
harness waits for control release and explicitly returns with Ctrl-] before the
creation flow. Neither failure was treated as product success. The fallback
rendering, multiline-paste, branch-only partial recovery and failed-intent-write
reconciliation checks were observed red before their corrections.

## Real native smoke and limits

The [raw native record](screens/m2-09-native-creation.json) and
[Claude](screens/m2-09-native-claude.txt)/[Codex](screens/m2-09-native-codex.txt)
cell captures show successful attributed native launch in distinct newly created
folders, exact branch/base commits and unchanged selected-owner identity.
Repeating the same Create retained each native session UUID. Both explicit Stop
operations confirmed Stopped; final Shutdown confirmed, and the owner exited 0.
All owned dashboard, fixture, native and supervisor processes were stopped before
the shared test slot was released.

Native folder trust held Claude at its ordinary trust dialog; Codex proceeded to
its hook-review dialog. The record's `trust_input_sent` only records the fixture's
input attempt; it does not claim that trust was confirmed. The first-prompt marker
was not visible, so these smokes establish native startup/cwd, deduplication and
cleanup, not fresh completed prompt turns or semantic-status qualification. The
fixture verifies exact prompt argv/cwd through this creation flow; the unchanged
adapter's actual native prompt behavior is qualified separately by the
[ticket 07 native proof](m2-07-prompt-naming.md). No trust or hook guard was bypassed
by product code.

## Checks and remaining qualification

Focused locked Git-worker/coordinator (6 tests), App and UI checks and the two
Controller/dashboard creation scenarios passed. The existing legacy-owner guard
has an additional isolated creation-protocol-zero case; it compiles and awaits the
merger's combined execution rather than an overlapping outer-PTY rerun. Stable
locked build, formatting and warnings-denied all-target Clippy are run on the
final feature source. The frozen combined all-target gate belongs to the merger;
no overlapping full/native aggregate is claimed here. Configured setup is ticket
10. Its interception point is the typed `LaunchHandoff { record, authorized_start }`:
the record retains original immutable requested intent for persistence; the separate
boolean alone permits live launch. Reconciliation always clears that authorization. Broader native prompt completion through trust/hooks, physical-terminal
compatibility, sleep/wake and actual reboot remain outside this scoped proof.
