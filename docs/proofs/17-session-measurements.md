# Paced session measurements — ticket 17

The final owned-child tracker source was independently revalidated with the full
workload: see [final tracker measurements](17-tracker-revalidation.md) for the
exact final revision, raw archives and updated latency/resource/storage results.

The scoped macOS proof passed: **200/200 active navigation samples were below
100ms; 100/100 active attaches reached a unique visible input ACK in under one
second.** Healthy durable completion intervals and oldest accepted ages stayed
below one second. Sustained history grew while memory reached bounded plateaus.

Full-storage backpressure still freezes native output. Dashboard navigation and
Stop remain available; a later fixture correction independently demonstrated
native input receipt while its visible ACK remained blocked until repair.
That censored ACK is **not** a healthy attach pass or full native usability.

## Sources, environment and artifacts

2026-11-06, Mac16,8 arm64, 48GiB RAM, 12 logical CPUs; macOS 26.2 (25C56), Darwin
25.2.0; stable rustc 1.99.1 (58cf8f9c3), Cargo 1.99.1; debug profile. The terminal
is portable-pty plus vt100 0.18.1, 160×40 outer cells; native TERM is
`xterm-256color`. There is no graphical terminal/font/physical-pixel timing claim.
Native Claude/Codex compatibility remains a separate versioned proof.

| Run | Exact clean source | Result | Standalone raw data, analysis and commands |
| --- | --- | --- | --- |
| Sustained final02 | `b2a43e1b816d8ca71259b863f84b2e80d83d5be9` | Passed, 438.28s | [17-final-02.tar.gz](17-final-02.tar.gz) |
| Fault-input supplement02 | `bc56e569999589816a322918f06c8dca7f86df38` | Passed, 34.03s | [17-fault-final-02.tar.gz](17-fault-final-02.tar.gz) |

[Artifact SHA256 checksums](17-artifacts.sha256) cover the successful and failed
archives. Both successful archives include manifests, exact binary SHA256, environment, raw JSONL,
producer/input ledgers, committed SQLite/chunk artifacts, public History evidence,
command logs, `analysis.json` and a standalone `ANALYZE.py`. The production session
prefix SHA256 is identical across both runs:
`1c288969564284179d54455a4540f6055d7e572743649948dc7b201e4ca18dd3`.
Only the scripted fixture's input flushing changed; its two exact binary/source
boundaries remain distinct. No healthy/resource numbers are silently reassigned
to the later fixture.

## Input to parsed visible feedback

The endpoint is the continuously consumed, terminal-parsed visible screen, not
an event enqueue or focus label. Every healthy attach also has an independently
timestamped unique fixture input receipt, the same managed identity/child, and
its entire injection-to-visible interval inside actual producer start/end times.
Fresh launch startup is excluded. Focus alone had a 7.234ms median and is not
used as the attach result.

| Scenario | n | p50 ms | p95 ms | p99 ms | Worst ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| No-output navigation baseline | 20 | 3.705 | 3.817 | 3.840 | 3.840 |
| Navigation with output and refresh | 200 | 7.135 | 7.785 | 8.027 | **8.375** |
| Attach through unique visible ACK | 100 | 461.615 | 610.392 | 626.896 | **661.666** |
| Navigation, 50ms slow append | 20 | 6.047 | 6.686 | 6.849 | 6.849 |
| Navigation, persistent ENOSPC (final02) | 20 | 4.623 | 6.202 | 6.979 | 6.979 |
| Navigation, ENOSPC (receipt supplement) | 20 | 4.887 | 8.271 | 10.064 | 10.064 |

Healthy attach receipt latency was median 283.924ms, p95 410.113ms, max 470.664ms;
receipt-to-parsed-visible tail was median 176.975ms, max 231.524ms. The separate
slow-storage attach reached its visible ACK in 897.564ms (one sample).

The corrected full-storage supplement recorded input receipt at **661.762ms**,
while focus appeared at 4.059ms and the visible ACK remained right-censored at
**2005.747ms**. The current native view was frozen; input receipt does not make
that view fully usable. Repair plus explicit Retry displayed the same ACK
124.225ms later. Final02 observed Stop before storage repair in 175.904ms. Its
2.151s retry-completion observation includes an intentional two-second harness
wait and is an upper bound, not a measured service latency.

## Sustained throughput and bounds

Parameters were declared before production: attached 150s; focused, fully
detached, actual nonreading subscriber and slow-storage phases 60s each;
then 30s persistent ENOSPC saturation. The detached window has neither dashboard
nor controlling socket. Slow subscriber saturation caused the actual transport's
500ms disconnect, followed by coherent same-session resynchronization.

Each lazy 1024-byte record contains run/sequence, cursor position/erase, green
SGR, lambda/CJK UTF-8 and X padding. One producer paces outside its short stdout
lock, without catch-up bursts. Offered rate is a target, not attempted bytes or
achieved throughput. The main thread collects bounded native input concurrently.

| Phase | Target B/s | Completed payload bytes | Achieved B/s | Max stdout acquisition/write ms |
| --- | ---: | ---: | ---: | ---: |
| Attached | 65,536 | 8,488,960 | 56,588 | 0.365 |
| Dashboard focused | 65,536 | 3,371,008 | 56,183 | 0.089 |
| Fully detached | 65,536 | 3,356,672 | 55,929 | 0.380 |
| Slow subscriber | 65,536 | 3,341,312 | 55,677 | 0.068 |
| 50ms append delay | 262,144 | 1,111,040 | 18,512 | 135.024 |

The slow phase spent 59.564s in producer stdout acquisition/write, establishing
real backpressure. Its reduced achieved rate is not a storage-capacity promise.
The receipt supplement completed a write blocked for **32.112s** after repair.
A terminated blocked write has no completed-write duration and is not invented
as a sample.

Normal capture has 64×4096-byte slots plus worker/producer ownership, at most
270336 pending raw bytes. Explicit Stop additionally reserves a FIFO of at most
256 blocks/1MiB; the conservative combined raw ceiling is 1318912 bytes, plus
fixed stack storage and bounded descriptors/session IDs. Exhaustion still blocks
and can leave Stop unconfirmed. Final02's observed pending high-water was
**68608 bytes**, with 1027 actual backpressure events and 91.943s cumulative
producer waiting. The largest encoded subscriber frame was **484519 bytes**
against the 4MiB cap.

Other declared limits remain 16 storage commands/events, 32 native-input/client
requests, one replacement client event, eight 4096-byte outer blocks, two
at-most-200×80 child screens, zero scrollback, 1024-byte OSC payload, 64KiB chunks
and 1024 commit records. The collector yields/reopens only at phase boundaries,
checks the same child PID/identity, and detects sequence gaps before ring wrap.
No unbounded test-output channel or historical record vector is used.

## Sampled memory and disk

There were 422 individual PID-attributed RSS samples. Values below are KiB, with
first/middle/last medians of equal ten-sample steady windows after five seconds
of warmup. They are sampled resident memory, not instantaneous allocation peaks.

| Phase | Supervisor first/middle/last | Child first/middle/last | Dashboard first/middle/last | Harness first/middle/last |
| --- | --- | --- | --- | --- |
| Attached | 7072 / 7552 / 7936 | 3472 / 3488 / 3488 | 7136 / 7424 / 7616 | 5008 / 5072 / 5072 |
| Focused | 8016 / 8176 / 8256 | 3536 / 3536 / 3536 | 7568 / 7648 / 7648 | 6480 / 6480 / 6480 |
| Fully detached | **8272 / 8272 / 8272** | 3536 / 3536 / 3536 | absent | 6960 / 6976 / 6976 |
| Slow subscriber | **10112 / 10176 / 10176** | 3584 / 3584 / 3584 | absent | 8992 / 8992 / 8992 |
| Slow storage | 10192 / 10272 / 10272 | 3584 / 3584 / 3584 | 9296 / 9568 / 9728 | 9488 / 9488 / 9488 |

The early supervisor growth fills the finite telemetry ring. The maximum styled
80×200 screen then increases screen/allocator overhead. Later history keeps
growing with flat detached/subscriber windows; the last ten slow-dashboard
samples are all 9728KiB. Full-storage first/last ten-sample supervisor medians
are both 10336KiB. Whole-run sampled peaks were supervisor **10480KiB**, dashboard
**9728KiB**, child **3584KiB**, harness **9584KiB**. Geometry/allocator warmup is
not proportional retained-history growth and is not hidden in a single total.

Logical transcript size grew from 100 to **20,347,181 bytes**, across **311**
chunks; sampled allocated transcript blocks ended at **20,348,928 bytes**.
SQLite files ended at 24576 bytes, with a 33296-byte sampled peak including the
transient journal. Test JSONL/archives are instrumentation, excluded from product
history/SQLite disk counts. Retained disk history is intentionally not bounded
by an automatic deletion policy.

## Actual durability and crash accounting

Every commit follows transcript `File::sync_all` and a successful SQLite FULL
transaction commit; SQLite fullfsync concerns SQLite's own files. Directory sync
is part of chunk creation. File close, written bytes and timer ticks are not
called durable. The ring recorded **1735 strictly advancing data-bearing
completions**, with no gaps. Failed appends no longer mint unchanged-prefix
completions. The table classifies intervals within actual healthy producer byte
ranges; idle gaps and injected faults remain separately visible in raw data.

| Healthy phase | Observed successive intervals | Max interval ms | Max oldest accepted age ms | Persistence B/s between observed commits |
| --- | ---: | ---: | ---: | ---: |
| Attached | 675 | 313.531 | 350.766 | 56,626 |
| Focused | 271 | 276.409 | 301.542 | 56,186 |
| Fully detached | 269 | 282.085 | 301.942 | 55,934 |
| Slow subscriber | 271 | 283.277 | 296.993 | 55,678 |

Healthy sync-plus-commit duration peaked at 58.578ms. Injected slow storage had
4.277s oldest accepted age; ENOSPC retained data for more than 32s. These are
fault results, not silently excluded healthy samples or one-second guarantees.

Completed producer payload totals **19,737,600 bytes**; controls, mosaics, ACKs
and startup output make the captured raw total larger. Every numbered payload
record is present exactly once. Final02's final accepted, written and durable
raw prefixes are all **20,347,181 bytes**, with pending zero and an actual clean
Shutdown acknowledgement after final flush and metadata commit. The supplement
separately retained all **95,401 accepted bytes** and 93 completed payload records.

The abrupt paced run's last acknowledged state had accepted/written **125084**,
durable **120988** bytes. After SIGKILL, the same identity restored Interrupted
with exactly **120988** readable bytes and identical before/after prefix SHA256
`e29466ebadfb9e4dbe53bfac3ab2ede31c1c95f226a9dca2a4d6c7c08b44e432`.
At least **4096 accepted, unflushed bytes were lost**. The final surviving producer
ledger has 126 completed records versus 118 recovered, so eight completed payload
records lie beyond recovery; four were already known accepted at the last
observation. More output can advance during the recorded request/response/kill
sampling gap, so exact accepted-at-kill loss is unknown. One at-most-1024-byte
in-flight producer write can also fall between stdout completion and its ledger.
No lossless emitted-output or universal crash-loss bound is claimed. Ticket 14's
older 1024-byte emitted-but-uncaptured TERM loss remains historical context.

Session/request IDs, authenticated owner/child PIDs and raw clocks are in the
archives. Reopen keeps identity, restores no coding child, refuses replacement
while execution is uncertain and refuses a confirmed Shutdown of that uncertain
record. Harness cleanup of its authenticated owner is explicitly separate from
product reconciliation. Public History checks cover start, rotation boundary,
final prefix and crash restore. Additional read-only SQLite/chunk streaming and
hashes supplement those public assertions; they never replace them or read
beyond committed extents. Process crash tests do not establish power-loss safety.

## Instrumentation, reproducibility and prior failures

The outer parser retains one bounded latest screen and polls predicates every
1ms. Its attached collector processed 3787 batches in 2.055s total, max 4.909ms
per batch; the focused collector processed 1173 batches in 0.631s, max 0.735ms.
The `ps` sampler sleeps one second after each sample; actual cost was p50
11.523ms, p99 23.439ms, max 65.678ms. Producer-ledger cost is recorded separately:
attached total 2.524s/max 23.155ms, other healthy totals 0.741–1.298s. None of
these costs are subtracted from latency results. Baseline navigation includes
application/harness scheduling overhead. There is no physical-display,
release-profile, actual full-volume, full-release-workload or product RSS/disk
capacity claim.

Use isolated checkouts of the exact source revisions above, stable Rust and new
unique output directories. Debug profile is required for the injected storage
markers; release silently disables them.

```sh
cargo build --locked
RAT_MEASURE17_DIR=/private/tmp/unique-main cargo test --locked --test ownership paced_session_measurement17 -- --exact --ignored --nocapture
python3 docs/proofs/17-analyze.py /private/tmp/unique-main
RAT_FAULT17_DIR=/private/tmp/unique-fault cargo test --locked --test ownership full_storage_input_receipt_measurement17 -- --exact --ignored --nocapture
python3 docs/proofs/17-analyze.py /private/tmp/unique-fault
```

The main archive's original revision uses the old fixture. The fault supplement
requires its recorded later revision. Current source can repeat both protocols;
new results must retain their own revision/binary rather than replace old data.
`RAT_MEASURE17_QUICK=1` selects short preflight parameters and cannot demonstrate
a sustained plateau. Final merged all-target/native validation belongs to ticket
18; this ticket's actual checks include formatting, all-target warnings-denied
Clippy, real-PTY red/green receipt and commit-accounting regressions, targeted
native-exit/final-seal and styled-Stop checks, successful short preflights, and the
two recorded final ignored measurement commands.

Prior failures remain reviewable: [preflight Stop failure](17-preflight-stop-failure.tar.gz)
contains actual EPERM/`?Es`, with the [causal bounded-drain fix](17-stop-drain.md).
[Partial run01](17-partial-run-01.tar.gz) retains the old 60-second attach window
crossing and generic eight-second completion-wait mistake; remaining resource
phases never completed. The corrected protocol reserves 150 seconds for all
100 samples and uses declared-duration completion budgets, collecting each phase
before the 1024-entry ring can wrap. Final02's old fixture flushed stdout after
every stdin byte, so its full-storage native input receipt was unconfirmed.
The receipt regression first failed, then passed after ordinary input collection
stopped acquiring stdout; there remains only one bounded line and one blocked
ACK. [Supplement collector failure01](17-fault-collector-failure-01.tar.gz) retains
the successful receipt/repair witness followed by a harness request after closed
Shutdown; the final supplement reads sealed History before Shutdown. No failed
collection run is promoted to a final pass.
