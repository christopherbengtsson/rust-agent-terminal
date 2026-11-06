# Final owned-child tracker revalidation — ticket 17

The final source passed the complete sustained workload again: **200/200 active
navigation samples below 100ms, 100/100 active visible-input attaches below one
second, all four healthy durable interval/age gates below one second, and later
resident-memory plateaus while retained history continued growing.** This run
includes the owned-child tracker introduced after the earlier ticket 17 proof.

## Exact boundary and reproduction

Both runs used clean tracked source `44b4d8c3997818358f62093b3d34e5215c373e08`,
including the final full process-identity Stop closure correction. The debug
application binary SHA256 was
`b86024843a42c53d470cdf9696b7ea850de134f5840ff198e0cfd65d9fc00f26`
in both manifests. No runtime or harness code changed for this revalidation.
The native-tool worker completed its four final Stop/quit runs before these
measurements; other project compiler/tests/native proof jobs remained quiescent.

Freshly recorded environment: 2026-11-06; Mac16,8 arm64, 48GiB RAM, 12 logical
CPUs; macOS 26.2 (25C56), Darwin 25.2.0; stable rustc 1.99.1 (58cf8f9c3),
Cargo 1.99.1. Debug portable-pty/vt100 0.18.1 outer terminal 160×40;
TERM xterm-256color; child initially 104×32, slow-subscriber maximum 200×80,
zero scrollback. No graphical-display latency or native CLI compatibility is
measured by this scripted fixture.

The tracker runs outside the UI loop with **100ms sleep after each inventory**;
its actual period includes process enumeration/identity checks. The workload
therefore measures its real effects on navigation, attach, memory and storage.
It does not establish a CPU budget or an isolated tracker-cost distribution.
The independent 1Hz RSS sampler costs p50 9.305ms, p99 13.818ms, max 14.843ms
(423 samples); this overhead is included in results, never subtracted.

| Run | Result | Standalone raw evidence |
| --- | --- | --- |
| Full sustained tracker final02 | Passed, 438.25s | [17-tracker-final-02.tar.gz](17-tracker-final-02.tar.gz) |
| Full-storage input receipt final01 | Passed, 33.96s | [17-tracker-fault-final-01.tar.gz](17-tracker-fault-final-01.tar.gz) |

Both archives include complete manifests/environment, exact commands, command
logs, `ANALYZE.py`, `analysis.json`, raw observations, input/producer ledgers,
committed SQLite/transcript artifacts and public History assertions. Checksums
are in [17-artifacts.sha256](17-artifacts.sha256). Use unique output directories:

```sh
cargo build --locked
RAT_MEASURE17_DIR=/private/tmp/unique-tracker-main cargo test --locked --test ownership paced_session_measurement17 -- --exact --ignored --nocapture
python3 docs/proofs/17-analyze.py /private/tmp/unique-tracker-main
RAT_FAULT17_DIR=/private/tmp/unique-tracker-fault cargo test --locked --test ownership full_storage_input_receipt_measurement17 -- --exact --ignored --nocapture
python3 docs/proofs/17-analyze.py /private/tmp/unique-tracker-fault
```

Parameters match the earlier [declared protocol](17-session-measurements.md):
150s attached, 60s each focused/fully detached/nonreading subscriber/50ms slow
append, then 30s ENOSPC; 1024-byte cursor/SGR/UTF-8 records, target 65536 B/s
healthy and 262144 B/s slow/full. No quick flag. The subscriber really saturates
and disconnects, then the same session resynchronizes. Synthetic ENOSPC is a
debug write marker, not actual full-volume evidence. Queue/screen/chunk caps
remain the declared protocol's bounds.

## Visible interaction and fault controls

The endpoint remains continuously parsed visible screen feedback. Each healthy
attach uses a unique fixture receipt/visible ACK and the same managed child;
all 100 complete injection-to-visible intervals are inside actual production.

| Scenario | n | p50 ms | p95 ms | p99 ms | Worst ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Baseline navigation | 20 | 3.635 | 3.883 | 3.898 | 3.898 |
| Output + refresh navigation | 200 | 5.607 | 6.291 | 6.418 | 6.559 |
| Attached unique visible ACK | 100 | 498.154 | 555.383 | 556.652 | 558.799 |
| 50ms slow append navigation | 20 | 5.421 | 5.904 | 6.440 | 6.440 |
| ENOSPC navigation, sustained run | 20 | 4.305 | 5.040 | 5.269 | 5.269 |
| ENOSPC navigation, receipt supplement | 20 | 4.407 | 5.749 | 6.010 | 6.010 |

The separate slow-storage attach took 896.455ms (one sample). Full-storage
visible ACK remained censored: the receipt supplement independently records
native input receipt at 648.391ms, focus at 4.193ms, and no visible ACK through
2005.586ms. Repair plus Retry shows that same ACK 117.703ms later. This is
**not a healthy attach pass or usable frozen native screen**. Dashboard Stop
was observed before repair in the sustained run after 288.299ms. Retry's
roughly two-second observation includes the intentional harness wait and is
not a service-latency measurement.

## Sustained resources and storage

| Phase | Completed payload bytes | Achieved B/s | Max completed stdout acquisition/write ms |
| --- | ---: | ---: | ---: |
| Attached | 8,493,056 | 56,619 | 0.052 |
| Focused | 3,380,224 | 56,321 | 0.048 |
| Fully detached | 3,328,000 | 55,461 | 0.070 |
| Nonreading subscriber | 3,099,648 | 51,650 | 0.046 |
| 50ms append | 1,068,032 | 17,798 | 163.392 |

The slow producer spent 59.510s in stdout acquisition/write: actual backpressure,
not a throughput capacity promise. Pending high-water was 69632 bytes, with
983 backpressure events and 91.852s accumulated producer wait. Largest encoded
subscriber frame was 484533 bytes below the 4MiB cap. Normal raw pending ceiling
remains 270336 bytes; explicit Stop's extra bounded tail permits the conservative
combined 1318912-byte ceiling. A forcefully terminated write has no completed
write duration; it is not fabricated as a sample.

RSS below is KiB, first/middle/last medians of equal ten-sample windows after
five seconds warmup, separately attributed by PID (raw archives retain PIDs).

| Phase | Supervisor | Child | Dashboard | Harness |
| --- | --- | --- | --- | --- |
| Attached | 7352 / 7744 / 8192 | 3384 / 3392 / 3392 | 6944 / 7184 / 7504 | 4984 / 5040 / 5040 |
| Focused | 8272 / 8544 / 8704 | 3456 / 3456 / 3456 | 7200 / 7408 / 7472 | 6464 / 6464 / 6464 |
| Fully detached | **8704 / 8704 / 8704** | 3456 / 3456 / 3456 | absent | 6928 / 6928 / 6928 |
| Nonreading subscriber | 10392 / 10616 / 10704 | 3504 / 3504 / 3504 | absent | 8960 / 8960 / 8960 |
| Slow storage | **10720 / 10720 / 10720** | 3504 / 3504 / 3504 | 9376 / 9824 / 9824 | 9488 / 9488 / 9488 |

Early telemetry-ring warmup and maximum-screen geometry cause measured growth;
the subscriber phase still rises 312KiB across its steady windows and is not
called flat. The detached window and later slow-storage supervisor window are
flat while history grows. Full-storage first/last ten-sample supervisor medians
are both 10752KiB. Whole-run sampled peaks: supervisor 10848KiB, dashboard
9824KiB, child 3504KiB, harness 9712KiB. These are sampled resident-memory
observations, not allocation maxima or product capacity budgets.

Logical transcript grew to **20,048,171 bytes**, across 306 chunks; allocated
transcript blocks ended at 20,049,920 bytes. SQLite ended at 24576 bytes, sampled
peak including journal 33296. Instrumentation artifacts are excluded from product
disk accounting. Retained history intentionally has no automatic disk limit.

All **1724** data-bearing completions strictly advance without sequence gaps.
Actual transcript sync and SQLite FULL commit precede each completion.

| Healthy phase | Successive intervals | Max interval ms | Max oldest accepted age ms | Persistence B/s |
| --- | ---: | ---: | ---: | ---: |
| Attached | 680 | 278.456 | 296.054 | 56,655 |
| Focused | 271 | 276.560 | 297.568 | 56,334 |
| Fully detached | 270 | 277.325 | 297.820 | 55,451 |
| Nonreading subscriber | 266 | 276.667 | 293.954 | 51,653 |

Healthy sync-plus-commit peaked at 26.152ms. Slow storage's accepted age reached
4.450s; persistent ENOSPC reached 32.620s. Those explicit fault results do not
establish a one-second failure guarantee.

Final accepted, written and durable prefixes all equal **20,048,171 bytes**,
pending zero, with a clean Shutdown acknowledgement. Raw prefix SHA256:
`2ecfd596b38e3a660738a9f60ee44e87e624e7398b23ef3ee349a921f76a344a`.
Every completed healthy/slow producer run has exactly its recorded payloads,
without gaps or duplicates. The stopped ENOSPC run has 68 captured records but
67 producer-ledger receipts: one 1024-byte write reached capture before Stop
interrupted its subsequent ledger receipt. This is the declared one-record
in-flight accounting uncertainty, not invented extra emitted evidence. The
receipt supplement separately ends with 91305 accepted/written/durable bytes.

The abrupt-run last acknowledged accepted/written prefix was 132252 bytes,
durable 131072 bytes. Recovery restored the same identity as Interrupted, exactly
131072 readable bytes, SHA256
`1fc8722e526f914906377bc573e9354d203b4677cce5529eb13e34f53aa939d7`,
identical before/after restore. **At least 1180 accepted, unflushed bytes were
lost.** Recovery contains 127 complete records plus 868 bytes of the next record;
the producer ledger contains 133 completed records. Counter/kill sampling permits
later output and one in-flight ledger uncertainty, so exact accepted-at-kill loss
is unknown. No automatic coding restart occurred; replacement launch and a
confirmed Shutdown of uncertain execution were refused. Harness cleanup of its
authenticated restored owner remains separate from product reconciliation.

## Gates and preserved early abort

All nine main analyzer gates and all three supplement gates passed. Build passed
on this exact source; the two ignored PTY tests and standalone raw analyzers ran
successfully. Final all-target/native validation is recorded by ticket 18.
These results establish the scoped one-session proof, not the full release
workload, native versions, power-loss safety or CPU/memory capacity promises.

[Early tracker run01](17-tracker-aborted-01.tar.gz) is preserved as an
**administrative abort**, not a product failure or final pass. It started at
`9cae355` after a ready notification, then was stopped when coordination required
the final Stop closure fix and native-proof quiescence. Its raw observations,
regular state files and log remain available; only its exact recorded child,
dashboard, supervisor and harness PIDs were signaled. Earlier historical ticket17
runs remain at their original source boundaries in the linked parent proof.
