# Milestone 2 ticket 05: retained projects and workspaces

2026-11-06, macOS 26.2 (25C56), arm64, stable rustc 1.99.1. Implementation
branch `feature/m2-ticket05`, starting at `bb306b3`, includes integration
`f63efd7` and the concrete AgentSession owner from ticket 03. This is registry,
migration and navigation evidence, not native Claude/Codex compatibility,
concurrent coding sessions, scalable history search or Milestone 2 completion.

## Implemented boundary

The existing elected supervisor and sole SQLite/transcript worker remain the
only execution/storage owners. Schema 4 adds projects, workspaces, explicit opaque
task aliases and UUID session membership. Canonical common directories identify
projects; common/Git-directory pairs identify checkouts independently of branch
or display name. Unknown legacy facts stay nullable. Original app path bytes,
session/native identities, request outcomes and transcript associations survive.
Registry changes commit transactionally before publishing rows. Registration has
an exact operation UUID and never changes the selected runtime or starts coding.

Migration reads explicit structured or bounded legacy identity facts, including
the legacy identity column when JSON has no label. It refuses conflicting or
unattributable records visibly rather than inventing membership. It preserves
opaque task/request strings, timestamp/seal/tombstone columns and raw transcript
files. No native recovery or launch is performed by migration, restoration,
registration, discovery or navigation. Existing lock election runs before any
storage open; an independent registry capability check precedes client commands.

The keyboard interface exposes `w` retained workspaces, `A` register an existing
local repository, `j/k` selection and `[/]` project jumps. Enter discovers the
selected project's worktrees, `a` explicitly adopts, `h` opens that workspace's
latest retained session by UUID, and `w`/Esc returns to the live pane. Unknown
legacy paths remain visible. Missing-path and storage errors retain previous
registry rows. Browsing is separate from the active coding target; the existing
single-runtime admission barrier still requires stopping before changing checkout.

## Observed checks

- `retained_registry_reuses_workspace_across_adoption_alias_and_restart`: real
  temporary repositories and supervisor protocol, A → B → symlink alias A uses
  A's original workspace identity; two projects/workspaces persist across restart.
  No coding request is sent and no execution identity appears.
- `populated_legacy_registry_migration_preserves_opaque_membership_paths_and_history`:
  populated schema 1, 2 and 3 fixtures, two distinct opaque tasks, requests with
  spaces, an absent JSON label, colliding lossy checkout displays, native IDs,
  missing/non-UTF-8 app path bytes, exact membership and readable transcript bytes.
  No repository fact is inferred from display strings. Existing request failures
  remain queryable. The absent-label extension first failed with one workspace
  instead of two, then passed using the explicit legacy column.
- `project_registration_deduplicates_linked_checkouts_and_failed_writes_keep_prior_registry`:
  real linked worktrees are one project/two workspaces, aliases deduplicate,
  injected ENOSPC at the real sole-writer registration boundary leaves rows and
  selection unchanged, explicit retry succeeds, missing paths retain records.
  Registration leaves adoption and execution empty.
- `registry_migration_refuses_unattributable_legacy_record_and_preserves_database_and_transcript`:
  visible storage-open refusal; byte-for-byte original schema-3 database and raw
  transcript remain intact after failed migration rollback.
- `dashboard_refuses_legacy_live_owner_before_sending_new_git_requests`: held
  ownership lock with legacy Git protocol 0 and Git protocol 1/registry protocol 0.
  Client sends zero unsupported bytes; candidate creates no SQLite database,
  preserves the owner, explains compatible-dashboard stop/upgrade and detaches.
- App checks preserve missing legacy selection and dispatch no launch from picker;
  the new picker transition first failed to compile before implementation.
  All 15 App and all 11 rendering checks passed after final help/context fixes.
  A 20-row render check selects the last retained record visibly at 160×40 and
  60×18, including unknown checkout facts and keyboard controls.
- Real dashboard PTYs exercise `A`, path entry, committed project appearance,
  explicit `a`, detach and reopening the retained selected workspace. Final
  normal/small case passed in 1.28 seconds. The initial harness incorrectly waited
  for typed path text rather than the committed row, then attempted adoption too
  early; its corrected wait targets the exact rendered registry row.

Build, formatting and warnings-denied all-target Clippy were run for this branch.
The combined all-target execution/session regression gate is delegated to the
integration merger after this branch releases its PTY slot; this document does
not claim that later aggregate result. Earlier bin aggregate had 32 pass, one
new help-overflow failure and one ignored terminal probe; focused final App/UI
runs passed after correcting the overflow. Existing PTY/native proofs are not
relabelled as new native validation.

## Visual comparison

[Normal real-PTY cells](screens/m2-05-registry-pty-160.json) and
[normal raster](screens/m2-05-registry-pty-160.png);
[small real-PTY cells](screens/m2-05-registry-pty-60.json) and
[small raster](screens/m2-05-registry-pty-60.png). PNGs were rendered with the
existing Menlo Swift cell renderer and visually inspected. These are real outer
PTY observations, not screenshots of a particular terminal emulator.

Compared with `VISION.md`, normal layout retains unavailable Reviews above the
worktree sidebar and the large session area. The picker temporarily uses that
area; leaving it restores the native pane. At 60×18 the sidebar remains hidden,
but the full-width picker shows selection/path and two lines of working controls.
The generic native help previously shown during compact selection was corrected.
Discovery context remains independent of registration/adoption so old worktree
results are never labelled as a newly added project. The reference's populated
review inbox, multiple simultaneous coding agents and chat-composer display are
not implemented by this slice. Captures respect the inherited terminal color
policy, so their monochrome rendering is not a new color/theme qualification.

Metadata snapshots project the retained registry as a collection; arbitrary-scale
pagination/resource qualification is not established here. History remains the
existing bounded escaped output reader; picker `h` chooses latest membership,
while selected older-conversation recovery and search belong to later tickets.
