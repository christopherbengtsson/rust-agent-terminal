# Post-review native quit verification

Two focused actual-native cases passed on clean source
`65ad6bbd8ab832f66613d7ba079b870c44860cba`: Claude Code **2.1.318** and
Codex CLI **0.167.0 --no-daemon**. This supplements the
[review corrections](18-review-fixes.md) and preserves the earlier milestone
[proof boundaries](18-milestone-evidence.md). It repeats native input/approval,
App focus/detach/reconnect/quit, additive typed identity, history and cleanup; it
does not rerun or reassign the historical complete native matrix or resource
measurements to the new source. Unexpected native nonzero-exit handling has
separate fixture/full-suite evidence; these cases request ordinary dashboard q.

## Exact execution boundary

2026-11-06, actual macOS PTYs, debug locked build. `cargo build --locked` passed
in 1.39 s before launching either case. Both ran the same prelaunch binary SHA256:
`a11e3153022624205efb620268715246b1140690929d10650de6b09bcfba72b9`.
The unchanged executed driver SHA256 was
`0d0a94757ca099ddfb64b8a5aadaebde8630bd2cb4b952d58f2a4b981aa7067e`.
Runtime/Cargo inputs were subsequently compared with final-check checkpoint
`677d272`: no difference. Its test-only missing-executable expectation correction
does not silently change this native source/binary record.

Fresh checkouts/state were `/private/tmp/rat18-review1-<claude|codex>-quit-checkout`
and corresponding `-state`, always passed with explicit `--state-dir`. Existing
authentication was reused. Proof-local PATH pins selected canonical Claude 318
(SHA256 677a2ea7bf576cedd20e191d973914ed8a41796f53a63b1923381ddc4d5b5e7d)
and Codex 167
(SHA256 030316ecfbb41f4e7946e584b0063ef333b7a36cd312ba349f2f6869e39b62cc).
The default Claude symlink/global configuration were unchanged. Normal native
folder trust and passive-hook review were accepted; command approval was once,
with no persistent allow or bypass. Only the bounded task-owned heartbeat script
ran, appending PID/PPID/PGID/sequence/time every 500 ms for at most 120 s.

## Outcomes

| Case | Native PID | Ordinary child PID / PGID | Heartbeats before detach→after q, then stable | Written=durable |
| --- | ---: | --- | --- | ---: |
| Claude 318 |88511|89023 / 89022|4→13|13759 bytes|
| Codex 167 |89514|90210 / 90210|4→9|32826 bytes|

Heartbeat growth includes the deliberate live detach, raw-protocol identity
inspection and same-session dashboard reopen before q; it is not a stop-latency
measurement. Both native and heartbeat PIDs disappeared. A subsequent
[exact-group audit](18-post-review-groups.json) found no remaining members of
these native/tool PGIDs; no signals were sent by that audit.

Before q, actual native PermissionRequest receipts and a nonempty native
conversation UUID were observed. Each structured `session_identity` contained
canonical task/session/request UUIDs and exactly derived the retained wire label
`task=… session=… request=…`. After ordinary q and supervisor restart all three
typed fields, the wire label and native conversation UUID were unchanged.
Both dashboard exits were 0 with exact termios restoration, Process Stopped,
execution null, committed metadata, pending capture 0 and written=durable.
Independent public History returned nonempty same-session history. Final
Shutdown was polled past its provisional response to confirmed stopped=true.
No user/default supervisor, credentials or shared daemon was modified.

[Exact typed/native identities and outcomes](18-post-review-native-summary.json),
[raw continuous outer-PTY streams, cell captures, observations and executed driver](18-post-review-native.tar.gz),
and [archive/file/driver checksums](18-post-review-native-manifest.json) retain
the full record. The archive was reopened and every captured file plus the
actual driver verified against its SHA256 before packaging intermediates were
removed. No raw child controls were replayed into the host terminal.

Both native workloads and all History/Shutdown tails completed before the review
worker's final all-target retry. A process audit found no owned proof dashboard,
supervisor, native CLI or heartbeat left. No compiler/native workload was run
by this proof worker during that retry. Its final receipt remains linked from
the review-correction record, with its own source/outcome boundary.
