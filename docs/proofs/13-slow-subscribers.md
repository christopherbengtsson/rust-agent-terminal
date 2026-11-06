# Ticket 13: bounded subscriber proof

Predeclared workload and caps (before resource runs): stable Rust debug build,
real temporary Git checkouts, real dashboard/supervisor/scripted PTYs on macOS.
Four modes: attached, dashboard focused, entirely detached, and a subscriber
whose actual socket is not read. Each emits 32768 numbered 128-byte records
(4 MiB), exceeding 16 times the capture queue and 64 transcript chunks. The
preliminary harness samples each PID's RSS separately; disk history grows.

Caps: two 200x80 screens, zero scrollback, 4096-byte PTY reads, 64 capture blocks,
one worker-owned block plus one producer block awaiting enqueue (270336 pending
owned bytes), plus the reader's 4096-byte stack buffer. At most one 65536-byte
raw chunk is open at a time; writes/sync/SQLite commit run in the sole storage
worker. Capture blocks wait on a full queue without discarding raw bytes; a
blocked-output child can still do other work. Input queue: 32 x 4096 bytes;
transport request queue: 32 bounded requests; observation queue: one finite
snapshot. Protocol frame/request buffer: 4 MiB; pending request accumulation can
overshoot by one 4096-byte read before rejection. Metadata/completion queues
have 16 slots; history pages use 2048 raw bytes with finite escape expansion.
Native status retains 16 tools and 16 summaries and reads <=32 receipts/pass.

Subscriber responses retain a single complete JSON frame and cursor, with
<=64KiB nonblocking writes per supervisor pass. A 500ms frame deadline closes
the socket and retains an explicit slow-dashboard reason for reconnect. Partial
frames are never followed by another frame on that socket. Other clients remain
refused promptly while control is held. New control after disconnect receives a
complete current screen and its owner-lifetime parsed-byte boundary; snapshots
replace the previous display rather than replay incremental output. Raw capture
is independently queued to persistence and never reconstructed from snapshots.
The UI processes at most one observation plus 32 Git events/frame and renders
only the bounded current screen.

Connection loss revokes semantic confidence and displays unknown process state
with last-seen lifecycle and retained identity. Press d to detach locally, then
reopen the same state directory; no launch is performed. q requires an actual
supervisor stop/flush acknowledgement and cannot falsely confirm shutdown.

This is preliminary scripted evidence, not native-agent support, a complete
capacity budget, storage fault proof (ticket 14), power-loss proof, or ticket
17's repeated measurement distributions. Terminal-parser visible timing is not
physical-pixel presentation timing.
