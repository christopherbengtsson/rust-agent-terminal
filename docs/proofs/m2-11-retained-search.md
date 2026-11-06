# Retained workspace and conversation search — ticket 11

Implemented source: `8844b88309cf13b8cff3f349474eb851e8fee382`, based on integration
`fb60d1a`. This scoped ticket proof covers metadata Find, stable retained-session
browsing, committed readable-history search and exact passage opening. It does
not establish the final Milestone 2 gate, six-session/reboot workload, selected
native Resume (ticket 12), or archive operations (ticket 16).

## Working behavior and bounds

Dashboard `/` searches all retained projects/workspaces by case-insensitive
metadata substring. `S` searches committed readable history across projects;
`B` browses retained sessions, scoped to the workspace when opened from its picker.
Tab changes mode; Enter submits edited text and opens a selected row; arrows select;
`n` obtains another bounded result page; Ctrl-R refreshes; Esc returns/cancels.
Editing consumes mixed-case/punctuation before global product shortcuts.

Metadata includes project/title/path and **last observed** branch facts from Git
discovery. Creation branch is not substituted for a current observation. Unknown
legacy project/path/branch facts remain labelled. Missing/inaccessible folders and
retained archive flags are visible without requiring a checkout for reading.
Archive fixtures seed the `workspace_history_facts(workspace,archived)` schema
contract; there is no archive control before ticket 16.

History uses all exact case-insensitive Unicode letter/digit words, punctuation
as separators, up to 8 words and 256 query bytes. Results contain at most 32 rows,
1000-character labels and 240-character snippets (plus truncation markers). One
query examines at most 32 keyset candidates per storage-worker turn and freezes
its highest passage/record rowid. The posting primary key seeks `(term,passage)`;
queries never replay a raw prefix or depend on unrestricted LIKE/FTS scans.
Long metadata/session scans yield, including when they have no matches. A new
query/cancel identity replaces the prior job; stale replies cannot replace the
current dashboard selection. A disappeared selected UUID remains unavailable
rather than choosing its neighbour. Reading changes no coding owner or native
conversation target and dispatches no Resume.

The sole writer incrementally indexes at most 512 committed raw bytes from one
round-robin retained session per turn, after capture and due flush. The parser’s
unfinished line is bounded to 2048 cells; CSI parameters and fragmented UTF-8 are
bounded. A slice can additionally emit that previously bounded line. Passage and
term SQL operations are therefore finite, including many short lines. Index
progress is persisted as full versioned parser state, not just a raw offset;
checkpoint/passages/postings commit atomically. Startup rebuild is incremental.
Older derived versions are pruned in bounded slices before replay. No generic
search engine, extra SQLite connection/writer or native provider was introduced.

Completed passages get stable `(session UUID, projection version, ordinal,
raw_start, raw_end)` identity. Exact open validates that identity and committed
extent, then reads the bounded persisted text directly. It does not assume that a
2048-byte raw page beginning at raw_start can emit the match. Unfinished live
lines stay outside search until completed, or final capture seals the tail;
results honestly say partial index and can be refreshed. Native full-screen text
remains an inert chronological projection, with repeated redraw fragments possible.
UTF-8, hidden terminal strings, controls and bidi formatting controls remain inert;
authoritative raw bytes are retained independently.

Deletion intent hides text immediately and cancels search. Every candidate/open
rechecks the retained tombstone and committed extent. Displayed results/passages
and editor-return snapshots are invalidated by a history revision; bounded
background pruning removes derivative postings/passages/checkpoints without
removing native IDs, launch receipts or session/workspace metadata. Restart cannot
make deleted text searchable again.

## Source-pinned active-output measurement

[Machine-readable measurement](m2-11-search-measurement.json) records the exact
command, host, binary SHA-256 and workload. Source `8844b88` was clean during the
run; binary SHA-256 was
`ce20c9785727088346a9b54c59906c6e7753ed76b23357e28f02e10624bc4510`.
Mac16,8 arm64, 48 GiB RAM, 12 logical CPUs, macOS 26.2 (25C56), stable Rust/Cargo
1.99.1, debug profile. Other agents yielded builds and behavioral tests for the
measurement window. This is a short fixture workload, not a native/resource or
sustained throughput measurement.

The actual Worker/supervisor/Controller/scripted-PTY path streamed three finite
`stream` commands: 600 fourteen-byte records plus three nine-byte prompts.
Repeated exact-word searches and cancellations ran while capture and background
indexing continued. The test collected the finite `DurableCommit` ring, asserted
strictly contiguous sequence identities (no gaps), checked each accepted-to-durable
age against one second, and waited for complete accepted output to become durable.
Runtime owner and session UUID stayed unchanged across all searches.

| Observed result | Value |
| --- | ---: |
| Completed searches | 65 |
| Maximum request-to-final-query observation | 53 ms |
| Contiguous durable receipts | 16 |
| Maximum oldest accepted age at durable commit | 289621 µs |
| Producer/completion observation window | 4362 ms |
| Whole targeted test wall time | 5.72 s |

An earlier test-development assertion wrongly subtracted
`oldest_accepted_micros` from `completed_micros`. The former already stores age,
not an origin-relative timestamp. Its apparent 1.044 s was a calculation error;
the actual receipt age was 290766 µs. The assertion was corrected before the clean
source-pinned run. No product flush failure is inferred from that arithmetic error.

## Keyboard, recovery and native-corpus evidence

The real dashboard test `keyboard_retained_search_opens_exact_passage_in_normal_and_small_ptys`
passed at 160×40 and 60×18, covering query typing/submission, search hit opening,
return, stable session row opening, metadata no-results and ordinary quit.
[Normal passage cells](screens/m2-11-keyboard-passage-160x40.txt) and
[small passage cells](screens/m2-11-keyboard-passage-60x18.txt) are actual
terminal-parsed captures. Session-list label clipping found during the first run
was fixed by wrapping; lifecycle wording differs in the compact header and the
fixture assertion now accepts that existing layout. Esc and quit are sent as
separate confirmed transitions to avoid the terminal interpreting Esc-q as Alt-q.

Compared with the vision image, normal size preserves reviews/project sidebar,
prominent right pane, status and keyboard footer. Reading temporarily replaces
the right native pane with search/results or a passage; small size hides the
sidebar as the existing responsive dashboard already does. This proof is terminal
cell/layout evidence, not a physical-font/pixel or native-agent screenshot claim.

Current search code also copied and migrated both archived ticket 08 corpora in
private temporary state, searched the known answer marker and opened its exact
persisted passage. These are new **search replay** checks of previously recorded
Claude/Codex output, not new native launches, native search timing or renewed
native compatibility claims. Original agent and native conversation ID survived
migration. Original source artifacts/tool versions remain in
[the 08 native proof](m2-08-readable-history.md). Replay outputs:
[Claude](screens/m2-11-HISTORY08-CLAUDE-ANSWER-search.json),
[Codex](screens/m2-11-HISTORY08-CODEX-ANSWER-search.json).

To reproduce, extract `m2-08-native-evidence.tar.gz`, set `RAT_HISTORY_CORPUS` to
`corpus-claude` or `corpus-codex`, `RAT_HISTORY_MARKER` to the corresponding
`HISTORY08-CLAUDE-ANSWER` / `HISTORY08-CODEX-ANSWER`, and `RAT_HISTORY_EVIDENCE` to
an output directory, then run:

```sh
cargo test --locked --bin agent-terminal \
  persistence::concurrent_tests::qualify_search_native_committed_corpus \
  -- --ignored --exact --nocapture
```

## Validation and remaining scope

Passed on source `8844b88`:

- `cargo build --locked`, `cargo fmt --all -- --check`, and warnings-denied
  all-target Clippy.
- App tests: 30; UI tests: 17; projection tests: 3; real Worker tests: 13.
  Two explicit corpus qualifiers are ignored in ordinary runs; the new search
  qualifier was separately run for both retained native corpora.
- Worker tests cover a raw passage spanning 9000-byte OSC, UTF-8, an uncommitted
  disk tail excluded from search, exact open after restart, deleted text absent
  after restart while session identity remains, real temporary Git metadata with
  missing/archived labels, and cancellation of a 1000-workspace no-match scan.
- A failed derivative transaction before commit rolls back passages and checkpoint;
  restart recovers split UTF-8 state and yields exactly one correct passage.
- App tests cover stale paste/query replies, stable reordered/deleted rows,
  editor-return deletion invalidation, and mixed-case global-key characters in
  project/prompt/rename/base/search fields. Existing creation-field regression
  remains passing.
- Actual targeted dashboard and active-output tests above passed. No unfiltered
  all-target/native/terminal-restoration suite was run concurrently with ticket 10.

The root integration runner retains ownership of the final assembled milestone
gate, migration/legacy rollback aggregate tests, native selected-resume and archive
flows, and combined workload/reboot validation. This scoped proof does not erase
the previously documented aggregate boundaries or label them passed.
