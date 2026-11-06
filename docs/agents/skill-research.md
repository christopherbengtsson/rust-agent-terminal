# Agent skill source assessment

Researched 2026-11-06. These are source inspections and compatibility judgments,
not executed evaluations. No candidate was installed, no candidate script was
run, and no external skill content was copied into the local skills. GitHub API
commit lookups and raw source reads established the pins below; search results
were used only to locate primary sources.

The relevant baseline is this repository's [product](../../PRODUCT.md),
[architecture](../architecture.md), [milestones](../milestones.md), and
[manifest](../../Cargo.toml): one Rust 2024 package, Ratatui 0.30.2, Crossterm
0.29, portable-pty 0.9, bundled rusqlite 0.37, and worker-owned blocking work.
The local glossary and ADRs govern lifecycle, native resume, history, and human
review submission. A generic skill cannot replace those contracts.

## Candidates and decisions

| Candidate and inspected primary source | Decision and compatibility |
| --- | --- |
| [blacktop/dotfiles: ratatui-tui](https://github.com/blacktop/dotfiles/blob/c4d613ca6b39ad73b3e0f7ab39031702dd7daa4e/ai/skills/ratatui-tui/SKILL.md) | **Adapt ideas into original rat-tui instructions.** Its 0.30.x and Crossterm 0.29 guidance matches this package. State/update/view separation and render-cost checks help. Reject template copying, optional animation/image dependencies, blanket release-profile changes, and its Claude-specific Workflow script. The existing terminal guard and bounded worker loop must remain authoritative. |
| [ZhangHanDong/ratatui-skills](https://github.com/ZhangHanDong/ratatui-skills/blob/b413430707a30cf1241c0598b3e4578526a5d009/SKILL.md) | **Reference only.** Root skill targets 0.30.0 and offers layout/widget navigation; that is close enough to inform API lookup, not a compatibility proof for 0.30.2. Its broad keyword trigger and mandatory shared defaults are less precise than a repository skill. Simple blocking event-loop examples do not establish responsiveness during managed sessions. |
| [cli/cli: gh](https://github.com/cli/cli/blob/205239fd8fa51df92ab47bd93b62aa2b230a07be/skills/gh/SKILL.md) | **Adapt selected patterns into original rat-github instructions.** First-party advice on structured output, repository targeting, pagination and API fallback is useful. Its broad current command catalog is not a minimum installed-version guarantee. Add this product's local-draft, exact comparison revision, explicit outcome/selection/preview/submission, stale-draft and uncertain-outcome rules. |
| [po4yka/rust-skills: rust-database](https://github.com/po4yka/rust-skills/blob/eae9ae3eb7ca3ef8c2895e44f38293b34fe21764/skills/rust-database/SKILL.md) | **Adapt a narrow subset into original rat-persistence instructions.** Existing-driver discipline, transaction ownership, commit acknowledgement, stable operation IDs and real-schema tests fit. Its pool/server/async-driver/deployment sections and assumed database test/advisory commands do not describe this sole-worker rusqlite design. Do not add those dependencies or targets to satisfy a generic checklist. |
| [RightNow-AI/OpenFang: sqlite-expert](https://github.com/RightNow-AI/openfang/blob/acf2587e46be174c10200489c9a2d23a39a98aeb/crates/openfang-skills/bundled/sqlite-expert/SKILL.md) | **Reject as a default skill.** It prescribes WAL, synchronous=NORMAL, 64 MB cache and 256 MB mmap settings without this product's durability or resource evidence. Atomic transactions and query-plan inspection can inform focused work, but blanket tuning and periodic vacuum must not bypass explicit retention or the measured storage contract. |
| [IronBee: terminal-testing](https://github.com/ironbee-ai/ironbee-devtools-skills/blob/4fa12f055a933a8348835bea952db66a4fc6d25e/skills/terminal-testing/SKILL.md) | **Adapt testing principles into original rat-sessions instructions; defer tool adoption.** Output predicates, screen capture, resize and explicit input/signals are useful. This skill requires a separate terminal CLI, can only drive processes it spawns, and permits idle waits. Silence cannot prove readiness, completion, or approval here. Its tool availability and descendant cleanup were not tested. |
| [obra/superpowers: using-git-worktrees](https://github.com/obra/superpowers/blob/22f0b463e7a5e579904dd6211a520321035526f7/skills/using-git-worktrees/SKILL.md) | **Adapt safety questions into original rat-worktrees instructions.** Detecting existing isolation, submodules and ignored placement is useful. Reject automatic .gitignore commits and permission-failure fallback to working in place for independent product tasks. Generic cargo setup lacks this repo's locked commands. Task creation, adoption, archive, checkout removal and history deletion need distinct contracts. |

All seven named sources were located. Decisions concern the inspected versions;
they are not security audits, endorsement of future updates, or license clearance
for redistribution. Original local instructions avoid vendoring candidate text.

## Scope of the five local skills

- **rat-tui:** trigger on UI layout, focus, key routing, rendering and terminal
  restoration work. Require state/event checks and normal/small viewport review
  against the vision image. An attached native Ctrl-C must keep native meaning.
  Do not turn a rendering request into session, persistence or async rewrites.
- **rat-sessions:** own PTY/supervisor integration, launch identities, reconnect,
  process ownership, resize and native state evidence. Keep execution lifecycle,
  semantic turn status, readable history and native resume distinct. Native
  compatibility claims require the actual CLI version and macOS evidence.
- **rat-persistence:** own the single writer, migrations, durable acknowledgements,
  transcript bounds, recovery, faults and explicit cleanup. Preserve accepted,
  written and durable distinctions, including the limits of backpressure and
  Stop. Never infer that SQLite metadata commit alone synced transcript bytes.
- **rat-worktrees:** cover product discovery/adoption/creation/removal separately
  from the agent's own development checkout. Discovery stays read-only. Use real
  temporary Git repositories, argument arrays and NUL-delimited records; preserve
  non-UTF-8 paths, dirty/unpushed work and retained history.
- **rat-github:** cover implementing/reviewing future GitHub workflows without
  implying those product features already exist. Fetch bounded structured data
  on workers, bind drafts to head and base/comparison revisions, and preserve
  human edits. Neither preparation nor navigating/editing a preview authorizes
  publication. Reconcile uncertain publication before retrying.

## Repository evidence that generic skills miss

[ADR 0003](../adr/0003-evidence-based-agent-status.md) requires unknown state when
native evidence is absent. [ADR 0004](../adr/0004-preserve-history-with-explicit-cleanup.md)
requires explicit history deletion and honest durability/failure reporting.
[Storage faults](../proofs/14-storage-faults.md) distinguish retained accepted
bytes from unread native/kernel output and durable prefixes. [Cleanup](../proofs/15-explicit-history-cleanup.md)
requires a final-capture seal, UUID-bound preview/confirmation, durable deletion
intent and inode-anchored safe unlink; stopped state alone is insufficient.

[Measurement evidence](../proofs/17-session-measurements.md) declares 64×4096-byte
capture slots, a conservative 270336-byte ordinary pending ceiling, bounded Stop
reserve, a 4 MiB subscriber frame cap, capped screen/parser state and a bounded
commit ring. These are implementation/evidence boundaries, not unlimited-lossless
or whole-release capacity claims. [Native child ownership](../proofs/18-native-child-stop.md)
records the separate-process-group failure and bounded live-identity correction;
process-name scans or signalling persisted PIDs would weaken that boundary.
[Integrated evidence](../proofs/18-milestone-evidence.md) limits native claims to
recorded versions/workloads and preserves failed runs and later retests. Linux
CI and scripted tools do not establish native macOS compatibility.

## Scripts and verification

No new skill scripts are warranted initially. Existing Rust integration tests,
proof drivers and capture/render utilities already exercise the difficult
mechanisms; a second harness would add cleanup and measurement obligations.
Skills should select the existing relevant checks and require evidence tied to
source, binary, native versions, isolated state, owned processes and workload.
Add a helper only after repeated use demonstrates a concrete missing operation;
keep it bounded and test its own cancellation/cleanup before relying on it.

Executed during research: read-only local file inspection, primary-source HTTP
fetches and GitHub commit lookups. Not executed: candidate examples/templates,
candidate CLI/daemon/workflows, application builds/tests, native session proofs,
Git mutations or GitHub publication. This document therefore establishes source
fit and proposed scope, not measured skill effectiveness or new runtime evidence.

## Local delivery and validation

The five original skills live in `.agents/skills/`, a repository discovery
location in the
[official Codex skill documentation](https://learn.chatgpt.com/docs/build-skills).
They keep automatic discovery enabled and can also be invoked by name, for
example `$rat-sessions`. No user-global skills or application dependencies were
changed. Installed `gh --version` returned 2.74.2; upstream trunk guidance is
therefore treated as reference rather than proof that its newer commands exist.

All five passed the skill-creator frontmatter validator, local path/skill-pointer
checks and relative Markdown link checks. The validator ran through an ephemeral
`uv run --with pyyaml` environment because system Python lacked PyYAML.
`git diff --check` passed.

Two independent read-only agent passes applied the skills to five requests:
lost launch reply followed by supervisor crash; faster history search with early
write acknowledgement; one-key prepare/approve/publication; Vim diff inspection
and native-input return; and worktree removal with unknown upstream/stopped state.
Their plans preserved the repository's recovery, durability and human-submission
contracts. Feedback added search work budgets/cancellation and explicit routing
from worktree cleanup to session ownership, plus fresh pre-removal checks.
These are behavioral plan evaluations, not executed feature implementations.
No Rust runtime changed, so Cargo checks and native/GitHub proof runs were not
repeated. A subsequent session's available-skill catalog included all five;
automatic selection across arbitrary prompts has not been exhaustively tested.

## Writing pass

Applied the [writing-for-agents guidance](https://www.aihero.dev/skills-writing-for-agents)
to the skill set: removed repeated product/architecture summaries, scoped checks
to affected boundaries, and used existing requirements/ADRs/proofs as conditional
references. Search scheduling and GitHub transport retain short branch-specific
references. Removed the redundant `AGENTS.md` catalog; skill frontmatter remains
the automatic-discovery source.

Independent agents applied the original and revised bundles to the same seven
requests: native key encoding, local review-choice serialization, streaming
history search/durability, Vim diffs/return, lost-ack crash recovery, review
approval/staleness, and removal without upstream evidence. Both produced plans
preserving the critical contracts. The revised bundle avoided unrelated launch,
CLI and layout investigation. It still overreached on model-only serialization
and encoder-only screen investigation; feedback narrowed the persistence trigger
and separated input from screen evidence. A separate writing reviewer found no
demonstrated loss of requirements. Duplicate CLI-help guidance was also removed.
The follow-up walkthroughs then selected only input evidence for the encoder
change and only the GitHub local-model path for serialization. Total skill text
fell from 2,677 to 1,621 words, including both references; entrypoints are 44%
shorter.

These are independent source-backed plan comparisons, not controlled runtime
benchmarks or proof of identical behavior. Format and reference validation passed;
runtime code and native evidence were unchanged. Further pruning should follow
observed use rather than adding speculative rules.
