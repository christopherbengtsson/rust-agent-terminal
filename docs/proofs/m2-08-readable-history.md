# Milestone 2 ticket 08: readable native history

2026-11-06, macOS 26.2 arm64 / Mac16,8 / Apple M4 Pro / 48 GiB,
stable rustc 1.99.1 and Git 2.50.0. This qualifies the readable derivative of
committed captured output, separately from naming and native resume.

## Source and evidence

Fresh actual Claude Code **2.1.319** and Codex CLI **0.167.0** runs used source
`c8cdcc0b3240e9486405335116c51d1d4830a77e`, debug binary SHA-256
`3ab5358ac380ed70b2a56f58e7377784c3b61d954e17440acd2252eff8c92cc4`.
This source includes the session collection and prompt/naming integration through
`1b33c54`. Later merges `8505ff8` and ticket 06 final `9a1dd26` add metadata/modal/fixture
corrections and target-safe App resize dispatch; they do not change `history.rs` or the projection/read scheduling. Final focused
checks ran after the final merge; no-child corpus replays ran after `8505ff8`. Projection-file SHA-256
is `35982114fbdd414604a7567e2d3a28fdf9f764638492e921670ad247e4ee8a96`.

The [evidence archive](m2-08-native-evidence.tar.gz) contains the exact driver,
outer PTY raw output and parsed cells, public controller observations/readable
pages, and copies of each source SQLite metadata file and its exact committed raw
transcript prefix. The [manifest](m2-08-native-manifest.json) hashes every file;
the [summary](m2-08-native-summary.json) records binary/tool hashes, source, session
UUID, committed prefix and owned-process cleanup. The [Claude](m2-08-claude-history.png) and [Codex](m2-08-codex-history.png)
monochrome cell rasterizations are supporting composition views, not Terminal.app/
iTerm screenshots, faithful color samples or compatibility claims.

Both runs used separate real temporary Git repositories and explicit private
state directories, the adopted-workspace launch path, native trust confirmation,
a first prompt, a separately authorized tool prompt, and one-time approval.
Only terminal output is recorded; the driver does not retain a raw input stream.
The script contains exactly `print(<marker>, flush=True)` and completes normally.
No native resume, conversation import or automatic naming decision produces the
readable output.

| Tool | Session UUID | Committed bytes | Recognizable command output |
| --- | --- | ---: | --- |
| Claude 2.1.319 | `b9984a05-7999-45dd-a1cf-c5e38aa26e8d` | 16,501 | `HISTORY08-CLAUDE-COMMAND-OUTPUT` |
| Codex 0.167.0 | `1e0c6f01-6f20-4080-9a98-59eb97028975` | 31,718 | `HISTORY08-CODEX-COMMAND-OUTPUT` |

The visible initial prompt asks the agent to say `HISTORY08-<TOOL>-ANSWER` and ask
whether to run the bounded output check, without running tools. Both answers
retain that marker and a natural question. The explicit follow-up asks for
`python3 proof-output.py` once and its exact output. Codex's readable tool passage
contains `Ran python3 proof-output.py` followed by the output marker. Claude's
completed-command view and answer retain the exact printed marker. These are
fresh native observations, not reused Milestone 1 evidence.

For example, the Claude marker passage maps UTF-8 text bytes to raw range
`[14769,14807)` in page starting at raw byte 14336; Codex's first complete marker
passage maps `[27066,27113)` in page starting at 26624. Public pages include the
stable session UUID, raw page range, committed extent, projection version 1, and
text-byte/raw-byte passage spans. Native output and later redraws can produce
several different passages containing the same text.

Actual 160×40 dashboard captures exercise `h` then the readable/raw `v` toggle.
The history action does not send Resume or start a child. Both runs explicitly
Stop, await final committed capture and metadata, verify the native PID is absent,
then Shutdown. Reopening the dashboard retains stopped identity/history without
starting coding. The supporting normal/60×18 TestBackend view keeps the existing
reference composition: reviews/tree/selection on the left at normal size, history
in the prominent right pane, status and actions below. Compact geometry hides
the sidebar as before. Retained terminal text replaces the active pane while
history is open; this is not a structured chat transcript/input control.

## Projection and scheduling contract

Raw storage and durability ordering remain unchanged. Escaped `HistoryPage.text`
is independently available through the actual raw-view action. Readable output
never writes raw chunks, executes escapes, opens captured links, modifies the
clipboard or copies keys/password input into history. Output can itself contain
sensitive information.

A worker read first returns the bounded escaped page with
`projection_preparing=true`. At most 16 pending jobs replay round-robin; each
worker turn reads/parses at most 32 KiB, then capture, control, cleanup and flush
retain their turns. A process-local cache contains at most 64 parser checkpoints
across all sessions. Cold distant reads can take many turns, but there is no
single unbounded prefix replay on the UI/storage loop. Checkpoints are derivative
and disappear at restart. No schema or durable write protocol was added.

Parser state carries UTF-8, escapes, control strings and the current line across
capture, raw-page and chunk boundaries. It holds at most 2048 scalar positions,
four UTF-8 bytes and 32 CSI parameter bytes; string payloads such as OSC clipboard
content are discarded while their parser state continues across arbitrarily many
pages. Unicode control scalars become replacement characters. Completed lines
carry stable source spans; the last-line preview is explicitly `provisional` and
must not be indexed as a finalized passage. Plain LF lines, CR/CRLF, backspace,
erase-to-end, SGR and horizontal/same-row cursor addressing have focused cases.

Every replay turn rechecks `history_state='retained'` and the committed extent.
Deletion intent evicts matching checkpoints; queued work cannot return deleted
content. Reader owner plus request UUID rejects late pages and errors after a
newer selection, including same-owner paging. Native conversation records and
checkout paths are not read to generate text. Final corpus qualification copies
only authoritative committed bytes into a fresh Worker with no checkout/native
conversation, flushes them, and reads the actual projected Worker result.

This is a terminal-output projection, not a historical screen emulator. Vertical
redraws preserve separate passages, so duplicate prompts, spinners, partial
updates, spacing and wide/combining-character geometry artifacts can remain.
Alternate-screen text can repeat. Unknown cursor operations delimit passages;
tabs become spaces; very long lines split at the bound. Invalid UTF-8 is replaced
in readable output while raw bytes remain intact. The page-end preview may be
redrawn later. These limits are visible in the retained native examples; exact
raw evidence remains available. Global history indexing, cancellation/query
scheduling across workspaces and search results remain ticket 11.

## Focused checks

- Projection cases passed for fragmented UTF-8/control/redraw, Unicode controls
  split between pages, and native horizontal/same-row addressing. The Unicode
  control and horizontal cases first failed with an unsafe C1 scalar and one-word
  lines respectively, then passed after the corrections.
- The history raw-toggle render/state test first fell through to native Resume;
  it now passes without any session effect at 160×40 and 60×18.
- Worker cases passed for OSC spanning 70 KiB and chunk/page boundaries, stable
  literal passage offsets, exact retained raw prefix, restart with no checkout,
  exclusion of an uncommitted tail, cancellation on deletion intent, and retention
  of another session. Existing concurrent capture/flush/fault/restore tests pass.
- The owner event seam passes late-result/error and wrong-owner rejection after
  a newer requested passage.
- Both final native corpora pass the explicit no-child Worker replay qualification
  after the session integration merge, with answer/command/output markers checked
  separately. Archived corpus metadata can be read without running native tools.
- Build, formatting check, warnings-denied all-target Clippy, projection tests,
  concurrent Worker tests, owner event test and UI tests passed. Aggregate all-target
  integration/PTY checks are coordinated by the root integration runner after
  tickets 06 and 08 merge, not run concurrently with this native proof.

To replay an extracted corpus without launching a child:

```sh
RAT_HISTORY_CORPUS=/path/to/extracted/corpus-claude \
RAT_HISTORY_MARKER=HISTORY08-CLAUDE-ANSWER \
RAT_HISTORY_OUTPUT=HISTORY08-CLAUDE-COMMAND-OUTPUT \
RAT_HISTORY_COMMAND=proof-output.py \
RAT_HISTORY_EVIDENCE=/tmp/readable-history-proof \
cargo test --locked --bin agent-terminal \
  persistence::concurrent_tests::qualify_readable_native_committed_corpus \
  -- --ignored --exact --nocapture
```

Use `corpus-codex` and `CODEX` markers for the other corpus. This is the early
readable-history qualification, not the full reboot, release workload or native
resume matrix required by later tickets.
