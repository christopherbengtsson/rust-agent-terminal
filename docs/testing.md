# Testing and opt-in qualification

Run the build, formatting, Clippy and all-target tests listed in the
[development instructions](../README.md#development). CI runs these checks on
macOS and Linux. `#[ignore]` excludes a test from the ordinary run; it does not
mean the test passed. Linux CI does not establish native macOS compatibility.

## The eight opt-in tests

This inventory records the Milestone 2 boundary. Feature owners must update it
when adding, changing or retiring an ignored test.

| Test | Why excluded from the ordinary run | When to run and future treatment |
| --- | --- | --- |
| `terminal::tests::terminal_probe` | Internal subprocess helper requiring a real PTY and `RAT_TERMINAL_PROBE`. | The ordinary `restores_terminal_modes_after_normal_error_and_panic` test drives it automatically. Keep the helper ignored and its parent in the ordinary suite; rerun the parent after terminal ownership or restoration changes. |
| `persistence::concurrent_tests::qualify_readable_native_committed_corpus` | Needs a captured native conversation and expected markers. | Run after readable-history, capture or persistence changes. Follow the [readable-history replay instructions](proofs/m2-08-readable-history.md). Add small sanitized fixtures to the ordinary suite in a future history change; retain full native corpus qualification separately. |
| `persistence::concurrent_tests::qualify_search_native_committed_corpus` | Needs a retained native corpus and expected search marker. | Run after indexing, search, retention or history restoration changes. Follow the [retained-search replay instructions](proofs/m2-11-retained-search.md). Move representative sanitized cases into ordinary regression tests while retaining larger corpus replay. |
| `editor::tests::installed_vim_opens_captured_source_and_comparison_positions` | Uses installed macOS Vim and needs exclusive editor qualification. | Run after source positioning, diff positioning or editor invocation changes. Keep portable position logic in ordinary tests; automate this test in a serialized macOS lane when installed versions can be controlled. |
| `macos_vim_and_pico_inspection_source_save_resize_and_supervisor_continuity` (`tests/editor.rs`) | Uses installed macOS Vim/Pico and owns a terminal slot. | Run after editor return, terminal restoration, save, resize or supervisor continuity changes, including review editing. Use the same future macOS lane as the Vim test. |
| `paced_session_measurement17` (`tests/ownership.rs`) | Multi-minute measurement requiring an isolated machine and evidence directory. | Run when changes affect single-session latency, resources or capture. Use the [measurement instructions](../README.md#development-measurement-proof) and [original proof](proofs/17-session-measurements.md). Keep as an explicit benchmark; a short preflight cannot replace sustained measurement. |
| `full_storage_input_receipt_measurement17` (`tests/ownership.rs`) | Full-storage timing supplement requiring isolated execution and evidence. | Run after input, storage backpressure, fault recovery or capture changes. Use the same measurement instructions. Keep as an explicit fault benchmark; do not assume the six-stream test covers every assertion here. |
| `six_stream_coordination_measurement19` (`tests/ownership.rs`) | Owns six streaming PTYs and requires exclusive runtime measurement with frozen binaries. | Run after coordination, PTY, capture, persistence, search, UI scheduling or diff-work changes that could affect the measured workload. Follow [current-source qualification](proofs/m2-19-review-fixes.md) and the [M2 qualification record](proofs/m2-19-qualification.md). Keep isolated from builds and other runtime tests. |

Corpus replay requires `RAT_HISTORY_CORPUS`, `RAT_HISTORY_MARKER` and
`RAT_HISTORY_EVIDENCE`; readable replay additionally requires
`RAT_HISTORY_COMMAND` and `RAT_HISTORY_OUTPUT`. Use the documented corpus-specific
values. Replay launches no native agent and cannot qualify a newer CLI version.

Run the installed-editor checks separately on macOS:

```sh
cargo test --locked --bin agent-terminal editor::tests::installed_vim_opens_captured_source_and_comparison_positions -- --exact --ignored --nocapture --test-threads=1
cargo test --locked --test editor macos_vim_and_pico_inspection_source_save_resize_and_supervisor_continuity -- --exact --ignored --nocapture --test-threads=1
```

For six-stream qualification, use a new, nonexistent evidence directory:

```sh
RAT_MEASURE19_DIR=/private/tmp/unique-m2-measurement cargo test --locked --test ownership six_stream_coordination_measurement19 -- --exact --ignored --nocapture --test-threads=1
```

Prebuild before taking the measurement lane, pin the source and binaries, and
run no other builds, PTY tests or native qualification concurrently. A blanket
`cargo test -- --ignored` is unsuitable: these tests have different prerequisites
and competing terminal/runtime ownership.

## Planning future milestones

Each milestone's validation plan must select required supplemental checks
before implementation, using the affected feature boundaries above. A required
check without its editor, corpus or machine is an unmet gate, not a pass.
Record the reason and remaining work explicitly. Historical evidence stays
attached to its recorded revision; it does not silently qualify later code.

Milestone 3 review preparation and editor work must include affected editor,
terminal, persistence and history checks, plus restricted-reviewer behavior
under concurrent coding load. Milestone 4 feedback and merge work must select
the affected Git, workspace, lifecycle and recovery checks. Milestone 5 must
measure the actual combined release workload (three projects, six coding
sessions, ten PRs and one reviewer) with real native tools and recovery gates.
The M2 six-stream fixture alone does not establish that release workload.

Report ordinary passed/failed/ignored counts separately from supplemental
results. For each selected supplemental check, record the source revision,
binary identity where relevant, OS/tool versions, command, evidence location,
result and limitations. List checks not rerun. Preserve failures and explain
what a later successful rerun establishes. Coordinate physical sleep/reboot
checks with the user; automated fixtures cannot substitute for them.

At M2 closure, the ordinary suite reported 278 passed and eight ignored. The
terminal helper was exercised through its parent. Installed-editor and
six-stream checks were separately qualified on the corrected production
source. Native corpus replay and the older M1 measurement supplements retain
their earlier evidence; they were not all rerun on the final M2 source. See the
[qualification record](proofs/m2-19-qualification.md) for exact boundaries.

## Adding or retiring an ignored test

Every new ignored test needs an explanatory `#[ignore = "..."]` and an entry
here covering prerequisites, a runnable command or linked procedure, rerun
triggers, feature responsibility, and a condition for automation or retirement.
Ignoring a failing regression is not a fix; diagnose it and retain the failure
in the validation record.

Tests need not remain ignored forever. Move deterministic fixture cases into
the ordinary suite as dependencies are removed. Add real-editor checks to a
dedicated serialized macOS job when practical. Keep resource measurements in an
isolated qualification lane. Retire an old check only after documenting which
replacement covers its assertions and which coverage, if any, is being lost.

## Native agent test costs

Before real-provider qualification, read [agent cost controls](agents/cost-controls.md).
The milestone workflow driver selects the shared test launch profile and checks
the supervisor and live sessions before sending prompts. A supervisor started by
an older driver must be safely stopped before beginning a fresh run. Use the
current binary built with `cargo build --locked --features native-agent-qualification`;
ordinary production builds ignore the test environment switch. Historical proof
archives retain their original settings.
