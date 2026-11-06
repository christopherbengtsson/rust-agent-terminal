# Working in this repository

Read the [vision](VISION.md), [product requirements](PRODUCT.md),
[architecture](docs/architecture.md), and [milestones](docs/milestones.md).

Use the stable Rust toolchain and Git. From the repository root:

```sh
cargo build --locked
cargo run --locked -- /path/to/repository
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

- Keep blocking process, Git, filesystem, and persistence work outside the UI
  loop. Workers return events; rendering and state transitions must remain fast.
- Make focused changes within one Rust package. Add modules before crates and
  avoid speculative frameworks or nonfunctional controls.
- Verify changed behavior meaningfully: use real temporary Git repositories for
  discovery, state tests for event handling, and PTY/native-tool checks for
  session integration. Report which checks ran and any unverified behavior.
- Keep architecture and user-facing instructions aligned with working behavior.
  Linux CI does not establish native macOS agent compatibility.
- Publishing a GitHub review requires an explicit user submission action after
  previewing the exact revision, outcome, and selected comments. Preparation,
  editing, navigation, or an agent's decision must never publish a review.

## Agent skills

### Issue tracker

Specs and tickets live as local Markdown under `.scratch/<feature-slug>/`.
Before reading or publishing specs or tickets, read `docs/agents/issue-tracker.md`.

### Triage labels

Tickets use the five default triage roles in their `Status:` line.
Before triaging, read `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout: root `GLOSSARY.md` and `docs/adr/`.
Before exploring the codebase, read `docs/agents/domain.md`.
