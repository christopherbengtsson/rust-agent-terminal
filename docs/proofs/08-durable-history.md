# Ticket 08: durable readable history

2026-11-06; macOS 26.2 arm64, stable rustc 1.99.1, debug build. Real outer PTYs
(160×40), real dashboard/supervisor/scripted child processes, temporary real Git
repositories and isolated private state under `/private/tmp`. The outer terminal
is the harness's vt100 parser. These tests do not establish a native CLI version's
history/resume compatibility, graphical presentation timing, sudden power-loss
safety, ENOSPC recovery, or the later full resource workload.

## Durable boundary

One supervisor-side worker owns bundled SQLite (rusqlite 0.37), with
`synchronous=FULL`, DELETE journal mode, and `fullfsync=ON`. Each raw chunk is
at most 65536 bytes. The worker writes output, calls `File::sync_all` on the
transcript, then commits its durable byte extent through a SQLite transaction.
Only after that commit does it update durable counters or complete a flush.
Chunk creation synchronizes the containing directory. Metadata success is sent
only after its transaction commits. The UI labels pending/failed metadata as
unconfirmed. The file primitive is Rust `File::sync_all` (fsync), not an explicit
transcript `F_FULLFSYNC`; SQLite fullfsync concerns SQLite's own files. Process
crash tests and successful system calls do not prove drive/controller behavior
under abrupt power loss.

The timer targets 250 ms between flush opportunities; healthy storage has a
one-second target, not a guaranteed maximum crash-loss bound. Stop confirms
managed process cleanup, capture drains to EOF, and quit waits for a successful
final flush plus lifecycle metadata commit. Failed final storage leaves quit
incomplete and retryable. A readonly-directory test actually prevents SQLite
journal creation, observes the visible write error and a live dashboard, repairs
permissions, retries q, and reads the retained final response after restart.

The capture queue holds at most 64 blocks of 4096 bytes (262144 bytes), with one
4096-byte reader block plus its transferred allocation, one at-most-4096-byte
worker pending block, and one current file handle. Append failures retain that
pending block, and a blocked sender retains its bytes outside the parser lock.
Control and completions have 16 slots each; metadata omits screen/parser/history
buffers. A readable page uses at most 2048 raw bytes, at most two chunk reads,
and finite escaped text expansion; the dashboard scrolls/page-navigates it.
The durable request-result cache is capped at 1024 records; capacity exhaustion
refuses further launches instead of evicting an outcome or enabling duplicate
execution. Restart does not clear these retained outcomes. The screen/subscriber
caps are ticket 04/13 responsibilities. SQLite caches,
OS buffers and allocator overhead are additional; this is not an RSS or total
disk promise. Disk history is retained without automatic deletion.

## Recovery and readable representation

Schema 0 creates metadata; schema 1 migrates launch outcomes to schema 2 in a
transaction. Higher unknown schema versions fail closed. Metadata retains task,
session, request, lifecycle, current adopted path bytes, and selected history
position. Launch outcomes survive restart. Active/stopping records restore as
Interrupted; no native child is spawned. Until ticket 09 reconciles execution,
replacement launch and changing an uncertain adoption are refused. Ticket 09
still owns durable pre-spawn intent, process evidence and all launch crash windows.

History uses committed byte extents even when a raw file contains an uncommitted
tail. Such tails are not acknowledged; extending that same file fails visibly
rather than silently counting or overwriting uncertain bytes. Lost unflushed data
is possible. Missing/truncated committed files cause visible errors. The abrupt
proof kills the isolated supervisor after a real durable acknowledgement,
restores the same identity as Interrupted, and reads its acknowledged response.
The harness separately cleans its known fixture child; that cleanup is not product
survivor reconciliation evidence. No automatic coding restart is claimed.

All captured output remains chronological, including erased text and alternate
screen content. CR appears as `\r`, ESC as `\u{1b}` and other controls as printable
escapes. For example `\u{1b}[2K` records erase-line and `\u{1b}[?1049h/l` records
alternate-screen entry/exit. These bytes never execute when viewing history.
Ordinary Unicode is readable; a page boundary or invalid UTF-8 can display a
replacement character, while the raw transcript remains intact. This is a
chronological readable representation, not a pixel-perfect historical emulator.
Keys/paste are not separately retained because password prompts can suppress
echo; output itself can contain secrets. Native conversation resume is separate.

## TDD and observed proof

The approved seams are real dashboard/supervisor/scripted PTY plus existing App
and rendering boundaries. The clean restart/missing-checkout test first failed
for absent restored lifecycle; the chronological test first failed for absent
fixture controls; the multi-chunk test first failed for absent numbered output.
Implementation made each pass. The multi-chunk assertion initially skipped the
first record because it shared a line with `fixture>`; fixing the independent
record parser established all 6000 records in order across at least seven bounded
chunks and sequential bounded pages. No transcript bytes were dropped to make a
screen or history reader keep up.

Preliminary streaming run: three queued `stream` fixture commands, ten-ms protocol
sampling, four-second observation window. Fifteen actual data-bearing completions
were observed (idle timer ticks do not increase the counter). Tuples are worker
monotonic completion ms, oldest accepted-byte age ms, transcript fsync plus SQLite
commit duration µs:

```text
(272,174,18762) (543,269,17857) (815,289,17919) (1090,286,18778)
(1365,287,19858) (1641,289,24103) (1909,292,17687) (2182,284,18222)
(2451,286,18594) (2723,289,17873) (2991,286,17142) (3265,290,18837)
(3538,291,18898) (3812,287,19959) (4086,292,18921)
```

Successive durable intervals were 268–276 ms, accepted ages 174–292 ms, sync/commit
17.142–24.103 ms. `output_bytes` is ticket 04's owner-lifetime parsed-screen offset;
`durable_bytes` is the current session's raw prefix. Measurements after relaunch
must baseline the former at session launch before comparing these counters.
The samples above use the first session, where those boundaries align.
The accepted timestamp is captured before the reader transfers
its owned block, so queue/backpressure time is included. The end timestamp follows
file synchronization and SQLite commit. This small debug fixture run meets the
one-second healthy-storage target; it is not the sustained workload/distribution
or storage-fault measurements required by ticket 17. A separate abrupt run
acknowledged 181 bytes, age 36 ms and sync/commit 18.721 ms, then recovered its known
response. That run proves the committed prefix, not a universal loss bound.

Reproduction (actual final results recorded after the final run):

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets -- --nocapture
```

Final merged-code suite: 17 unit tests passed (one ignored subprocess probe is
covered by the terminal restoration driver), 19 ownership/history/native-choice
scenarios passed, the scripted interaction scenario passed, and the maximum-size
screen scenario passed. Maximum encoded 200×80 screen snapshot remained 358592
bytes. Build, formatting and warnings-denied all-target Clippy passed. The final
suite's additional healthy run observed 15 data-bearing completions: accepted
ages 173–302 ms and sync/commit 15.628–29.236 ms; all observed successive flush
intervals remained below one second. The quoted preliminary samples above are
retained as a separate run rather than silently replaced.

The last exact-code all-target rerun also passed (17 unit, 19 ownership/history,
one scripted interaction, one screen integration). Its healthy accepted ages were
176–298 ms, actual sync/commit 16.149–30.535 ms, and all observed data-bearing
completion intervals stayed under one second. A final capacity-error wording
change removes obsolete advice that restart clears the now-durable request cache.
