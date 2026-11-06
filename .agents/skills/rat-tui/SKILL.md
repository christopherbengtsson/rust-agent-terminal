---
name: rat-tui
description: Implement dashboard layout, keyboard focus, navigation, or external-editor return in this Rust terminal project. Use for Ratatui rendering and Crossterm interaction changes.
---

# Dashboard interaction

Paths are repository-relative.

1. Identify the changed transition in `App` (`src/app.rs`), its effect vocabulary
   (`src/events.rs`), or its rendering (`src/ui.rs`). Locate the owner and the
   visible result before editing; inspect `src/main.rs` when dispatch changes.
2. Use the branch below to select evidence. Implement through the existing
   interface and verify the changed transition, including rejection or failure
   when the change affects dispatch. A help/style edit needs rendering evidence;
   input bytes and terminal ownership need the PTY seam.
3. Report what changed, which checks ran, and what remains unverified. For a
   usable layout change, compare normal and small viewports with the vision
   image and explain material deviations.

## Branches

- **Execution interface:** when changing request/observation identity,
  disconnection handling or editor artifact consumption, read the
  [execution-machine boundary](../../../docs/architecture.md#execution-machine-boundary-direction-local-execution-today).
- **Layout:** inspect `Ui::session_viewport` before changing geometry shared with
  PTY resize. Read [screen evidence](../../../docs/proofs/04-session-screen.md)
  for interior bounds, unusable sizes, wide cells, styles and cursor checks.
  Use `TestBackend` tests in `src/ui.rs`; inspect the
  [vision image](../../../docs/images/conductor-dashboard-v3.jpg) for composition.
- **Focus/input:** inspect `App::input`, `App::paste` and `src/native_input.rs`.
  Read [native-input evidence](../../../docs/proofs/03-native-input.md) for focus,
  shortcuts and paste-mode behavior. Verify App transitions; use `$rat-sessions`
  when changing actual native input or resize delivery.
- **Terminal/editor handoff:** inspect `TerminalGuard` and its PTY restoration
  test in `src/terminal.rs`. Read the keyboard/diff requirements in
  [PRODUCT.md](../../../PRODUCT.md) for editor return and retained context;
  use `$rat-worktrees` for diff preparation. Verify release/reacquisition and
  failure return with the configured editor before claiming native support.
- **Visual qualification:** read the visual review in
  [integrated evidence](../../../docs/proofs/18-milestone-evidence.md) for recorded
  captures and their limitations.

For API questions, resolve versions/features from `Cargo.lock` and `Cargo.toml`
then consult the matching Ratatui/Crossterm docs. Keep terminal ownership with
this application's guard when adapting upstream examples.
