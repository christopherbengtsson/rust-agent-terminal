# Paced full-storage Stop regression

On native macOS, the ticket-17 quick measurement at reproduction checkpoint
983a2c8 reproduced `StopFailed`: EPERM from signalling an exiting PTY leader
(`?Es`), not a durable-metadata or dashboard projection failure. The local raw
run `/private/tmp/rat17-stop-debug01/observations.jsonl` records child 37258,
accepted 1710205, durable 1642621 and pending 67584 bytes. Repairing capture
allowed the same Stop to complete. Ticket-17 preflight 02/03/04 independently
recorded this symptom before this investigation.

Ranked predictions were: blocked terminal exit needs PTY drain; group verification
fails independently; or the dashboard has stale state. The causal probe changed
only capture ownership during Stop. Lifecycle TERM/KILL deadlines, actual child
reaping and live-group verification remain unchanged. The sole reader retains a
finite FIFO tail instead of blocking at the storage channel immediately. Original
quick workload then passed in 41.20s (`/private/tmp/rat17-stop-drain01`), with Stop
visible before repair. Peak pending became 68608: only 1024 extra bytes were
needed. After repair accepted and durable both reached 1709179, pending zero.
This supports terminal-output drain dependence; it does not infer success merely
from an exiting/zombie process label.

The minimized regression is
`paced_capture_saturation_stop_confirms_without_storage_repair`, through the
approved real dashboard/supervisor/scripted PTY seam. A maximum styled mosaic,
resize back, then saturated failing capture reproduces the failure. The earlier
five paced phases, input ACK and its two-second delay are unnecessary. Without
the mosaic, focused checks stayed green even after seven seconds of saturation,
ACK input and live resize: those alone were a false-negative seam.

Red check on the integrated cleanup/telemetry source disabled just the two
`stop_drain.store(true, ...)` activations (diff inspected before running). The
minimized command failed in 10.18s with exact EPERM, accepted 670836, durable
603252, pending 67584. Restoring the activations passed in 2.79s, including
pending-history visibility, cleanup ineligibility before repair and
accepted==durable after repair.

```sh
cargo test --locked --test ownership paced_capture_saturation_stop_confirms_without_storage_repair -- --exact --nocapture
RAT_MEASURE17_QUICK=1 RAT_MEASURE17_DIR=/private/tmp/unique-new-evidence cargo test --locked --test ownership paced_session_measurement17 -- --exact --ignored --nocapture
```

Reserve policy: at most 256 additional blocks, each <=4096 bytes (1MiB raw),
with bounded descriptors and per-block session IDs. No second reader, byte
reordering, terminal flush/discard or PID-only signalling is introduced. Accepted
bytes are counted once when read; retained ownership transfer does not accept
again. Exhaustion blocks the reader and genuine bounded group-verification
failure remains `StopFailed`. The EOF receipt is sent only after every retained
block transfers to the worker. Existing final durable-tail seal, replacement
launch and cleanup guards remain in force. A process can still perform work
while ordinary capture is backpressured; no safe-suspension claim is made.

This is a scripted native-PTY regression, not a new Claude/Codex compatibility,
physical display latency, actual disk exhaustion or full-release capacity proof.
