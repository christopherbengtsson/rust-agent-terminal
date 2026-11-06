# Milestone 1 integrated evidence — ticket 18

2026-11-06, Mac16,8 arm64, macOS 26.2 (25C56), stable Rust/Cargo
1.99.1, debug profile. This is one repository and one active native session at
a time, with real macOS PTYs. It does not establish C1–C9/R1–R11, the combined
release workload, a physical reboot or graphical Terminal.app/iTerm compatibility.

## Current gate

**Native foreground Stop and ordinary-quit retests passed for both tools on
`a3842906fddfc5649af11a898f987785dc0e7ece` (includes integration `a39308b`).**
The original actual Codex failure remains [preserved](18-stop-gate-failure.json).
Final source-followup checks and resource/latency revalidation remain open;
Milestone 1 and ticket 18 are not yet declared complete.

## Source and native boundaries

Initial actual native supervisors ran clean integration
`4ec3a8b6d7d2dc87050ed4bba3dfdca19df8edf3`. Later rebuilding changed only four
Codex reason strings, replacing misleading “support gate open” with bounded
evidence/version descriptions. Already mapped supervisors retained the earlier
wording; hook helper processes resolved the rebuilt path. No classifier or
process policy changed in that cosmetic delta. [Rebuilt binary/native hashes](18-build-boundary.json)
record that distinction; no prelaunch hash was captured for the original mapped
binary. Native support is attributed to the exact source and displayed native
version, not retrospectively to the rebuilt path.

Claude was pinned only through a proof-local PATH symlink to the canonical
`~/.local/share/claude/versions/2.1.318`, SHA256
`677a2ea7bf576cedd20e191d973914ed8a41796f53a63b1923381ddc4d5b5e7d`.
The default auto-updated 2.1.319 symlink was preserved and is untested. Codex was
canonical 0.167.0, SHA256
`030316ecfbb41f4e7946e584b0063ef333b7a36cd312ba349f2f6869e39b62cc`,
launched with `--no-daemon`. Existing native authentication was reused; normal
folder trust and passive invocation-hook review were accepted. No credentials
were copied, global policy edited or bypass flag used.

The fresh empty Git checkouts were `/private/tmp/rat18-claude-checkout` and
`/private/tmp/rat18-codex-checkout`; private explicit state directories were
`/private/tmp/rat18-claude-state` and `/private/tmp/rat18-codex-state`. Initial
restricted runs failed to create the supervisor socket and are not native passes;
authorized unsandboxed runs used only these proof-owned surfaces.

## Integrated native observations

The real dashboard discovered each initially empty checkout without launch,
explicit `a` adopted it and `c`/`x` launched. Native bracketed paste of
`Reply exactly: native-proof λ 界. Do not use tools or write files.` produced
the exact Unicode answer and bounded Completed observation for both tools.
Ctrl-] changed focus, Enter reattached, `d` detached and reopening retained the
same task/session/request/native ID and PID. Native questions used Claude
AskUserQuestion and Codex `/plan` plus request_user_input; Enter chose alpha.
Codex default-mode request_user_input_async produced conversational choices,
not the qualified numbered modal, and was not counted as input-needed evidence.

Claude manual mode displayed a real Bash permission modal for
`printf approval-proof > approval-marker`; Yes was selected once. Both tools
displayed a one-time native permission for the harmless bounded foreground
`python3 proof-heartbeat.py`; no persistent allow option was selected. Repeated
attached and dashboard resizes used 100×24, 60×18, 160×40, 120×28, 160×40
(outer cells); the normal child pane was 104×32. Narrow/wrapped modals may stay
Unknown and need native scrolling; no narrow status-support expansion is claimed.

Native Ctrl-C was driven during actual active generation. Claude required
waiting for streamed ⏺ output before sending Ctrl-C; pre-output cancellation
was not counted. Codex produced its actual Interrupt hook. Both resulting
observations were Interrupted with native acknowledgment and retained processes.
Earlier timed attempts that completed before Ctrl-C are retained as non-passes.
Killing only each dashboard preserved its native PID; reopening started no
coding and native `/exit` then produced Exited with retained history.

### Actual foreground tool Stop

The bounded heartbeat appended its PID, PPID, PGID, sequence and time every
500ms for at most 120 seconds, in its isolated checkout only. It used ordinary
native command execution, with one-time user approval.

Claude CLI 72091 started zsh88600/PGID88600 and Python88602/PPID88600, PGID88600.
Stop observed 70 records before, 72 after and then stable; both known child PIDs
were absent, lifecycle Stopped, execution null, error null. This separate native
child group was cleaned by graceful Claude termination in this scenario.

Codex CLI 72113 started Python88651, PPID72113, PGID88651, native start
17:42:00. Stop killed the CLI and reported Stopped/execution null/error null, but
Python became PPID1 and continued: 5 records before, 13 after and still growing.
This false successful Stop required the correction and fresh retests below. No unrelated shared daemon
or user process was signaled.

## Consolidated independent evidence

| Requirement | Exact evidence and limit |
| --- | --- |
| Five states plus Unknown | [Claude318](10-claude-status.md), [Codex167](12-codex-status.md): correlated passive native hooks plus current qualified UI, bounded age/generation; stale, invalid, quoted and continuation negatives retained. Missing evidence is Unknown. |
| Launch/crash/ack ownership | [Reconciliation](09-launch-reconciliation.md): durable prepare/authorize gate and recorded request results; uncertainty blocks replacement. Injected boot fixtures are not physical reboot. |
| Native original conversation resume | [Both actual native tools](16-native-recovery.md): original-ID marker recall after normal exit; missing checkout does not remove readable history. Final all-target tests retain original-ID/recovery coverage. |
| Storage faults and cleanup | [Storage](14-storage-faults.md), [explicit cleanup](15-explicit-history-cleanup.md): accepted/written/durable distinctions, bounded backpressure, explicit UUID-bound deletion/tombstones; legacy unknown age/unsealed history is not silently eligible. |
| Sustained resources and latency | [Exact sources/raw replay](17-session-measurements.md): 200 active navigation max8.375ms; 100 usable attach max661.666ms; healthy commit interval max313.531ms/oldest accepted350.766ms. RSS plateaus with 20,347,181 raw bytes durable, not a product capacity guarantee. |
| Fault interaction/loss | [Ticket17 supplement](17-session-measurements.md): receipt661.762ms; visible ACK censored2005.747ms, appears124.225ms after repair; all95,401 accepted bytes durable. Frozen native output is not fully usable. Abrupt loss recovered120,988 durable bytes and lost at least4096 accepted/unflushed bytes. Earlier ticket14 TERM1024 emitted-but-uncaptured loss belongs to its historical source before the bounded Stop tail. No universal emitted-output/no-loss guarantee. |

## Visual review

Whole real dashboard outer-PTY streams were bounded at 8 MiB per invocation,
parsed with vt100 0.18.1 and rasterized through the existing
`scripts/render-terminal-capture.swift` seam. These are cell rasterizations, not
graphical terminal screenshots. Raw child controls were stored as data and never
replayed to a user's terminal. Only isolated proof surfaces were captured.

Compared visually with [VISION reference](../images/conductor-dashboard-v3.jpg):
normal 160×40 retains Reviews above the project/worktree tree on the left and a
prominent native session on the right, readable identity/status/focus and
keyboard actions. Reviews honestly says unavailable; this milestone exposes no
review/diff/publication controls. One active session per worktree replaces the
reference's multiple agents. The native CLI replaces the mock transcript/product
composer. At 60×18 the sidebar is hidden, retaining abbreviated identity, focus,
process/status and escape/help; native content can clip and needs native
scroll/navigation. Tiny unusable viewports preserve the last usable child size
(the passing render test verifies no invalid resize request). Full later history
search, archival, editor, multi-project and review workflows remain deferred.

## Required checks actually run

On 4ec integration plus the four cosmetic reason strings: locked build passed;
all-target warnings-denied Clippy passed; formatting initially reported the new
long-line wrapping, `cargo fmt --all` corrected it and fmt check passed.
`cargo test --locked --all-targets` passed: 26 unit tests; one ignored terminal
probe exercised by its passing restoration test; 50 ownership scenarios in
357.50s, two ignored separately measured ticket17 workloads; scripted native-style
interaction/status 26.76s; screen integration 8.34s. Test fixtures are not native
compatibility evidence. Final source after the Stop fix needs its own checks.

Exact termios before/after matched for normal detach/native-exit/Stop drivers.
SIGKILL dashboard loss cannot run restoration and correctly recorded false;
reopening used a new PTY. Passing restoration tests cover normal/error/panic,
not restoration by a process after SIGKILL.

### Actual native supervisor failure and ordinary idle quit

After native exit, explicit fresh launches and ordinary `q` passed for both
versions: dashboard exit0 and exact termios restoration. Reopening and another
explicit launch were separate actions; startup did not itself execute coding.
This idle quit does not close the foreground-tool quit gate.

On the rebuilt binary hash in18-build-boundary.json (only cosmetic reason delta),
a new idle Claude native PID90402/conversation87eb96a6-9536-41da-aa6c-32aa3d1e1cc9
and Codex native PID90345 were active when only verified task-owned supervisors
90380 and90295 respectively were SIGKILLed. No commands or file writes were
active. Codex had not yet supplied a native conversation ID in that invocation;
the retained null is not invented as an ID. Both dashboards showed connection
failure/last-seen state and detached normally with restored termios.

Reopen restored exactly the same managed identities as Interrupted with semantic
Unknown and readable `h` history. Explicit new `c`/`x` was refused because
execution was uncertain; no coding child started. Exact native PIDs90402/90345
and the old bounded-heartbeat PID88651 were subsequently absent. This absence
does not prove no escaped descendants, so same-boot uncertainty remains reserved.
No injected boot marker, reconciliation bypass, physical reboot or automatic
resume was used. Restored proof-only owners are disposed separately from product
reconciliation after preserving this evidence.

## Reviewable captures and reproducibility

[Initial native raw streams, observations, cell JSON and drivers](18-native-initial.tar.gz)
are checksummed by the [archive/file manifest](18-native-initial-manifest.json).
The driver has deliberately isolated hardcoded temporary paths: inspect and adapt
all binary/state/checkout/output paths before reproduction. Never run it against
the default supervisor. The Rust helper parses outer raw bytes as data; the Swift
renderer creates PNGs from those cells. Source/viewport/mapped-binary distinctions
above govern the archive; its convenience source string is the base revision,
not a claim that an already mapped supervisor changed on rebuild.

| Whole dashboard capture | Observed boundary |
| --- | --- |
| [Normal native input](screens/18-claude-question-dashboard.cells.png) |160×40; actual input-needed label and native question; Reviews unavailable/tree visible.|
| [Normal native approval](screens/18-claude-approval-dashboard.cells.png) |160×40; exact command and one-time choices in native pane.|
| [Small Claude question](screens/18-claude-small-attached.cells.png) |60×18; attached identity/focus/actions; aged evidence correctly Unknown; clipping requires native navigation.|
| [Small Codex question](screens/18-codex-small-attached.cells.png) |60×18; native numbered choices remain visible; long footer clips, dashboard escape stays visible.|
| [Supervisor connection failure](screens/18-claude-supervisor-failure.cells.png) |160×40; last-seen Running identified as unknown, actionable local detach guidance.|
| [Interrupted replacement refusal](screens/18-codex-replacement-blocked.cells.png) |160×40; uncertainty/reboot guidance, no automatic child.|
| [Normal cleanup](screens/18-claude-normal-cleanup.cells.png) |160×40; sealed stopped histories eligible; current interrupted segment unavailable.|
| [Small cleanup](screens/18-claude-small-cleanup.cells.png) |60×18; UUID/path wraps, keyboard-only preview/cancel remains; fewer rows fit.|

Every linked PNG was visually inspected against the reference. Text states and
focus are readable without color. The material visual deviations and narrow
consequences above are accepted scoped product differences, not omitted features
presented as controls. A first cleanup driver attempted `d` while still inside
the cleanup pane and timed out; the corrected driver uses Escape back then
detaches. No history deletion occurred. A Swift module-cache sandbox denial was
rerun with authorized cache access; rasterization then passed.

[Initial proof-process disposal](18-initial-process-cleanup.json) records exact
UID/start/argv for both restored owners and their honest refused Shutdown
(stopped=false) before task-harness-only SIGTERM. Both owners and all known
native/heartbeat PIDs were then absent. This disposal is not product
reconciliation, and preserved uncertainty was not cleared or history deleted.

The interruption driver's search for fresh streamed output cleared its outer
collector before the prompt. Its retained native observation proves the actual
acknowledgment, but the corresponding incremental raster lacks the initial full
dashboard frame and is excluded from whole-dashboard screenshots. The archive
retains the raw/observation boundary; no missing cells were reconstructed from a
different invocation. The other linked whole captures retained their initial
frame.

## Native foreground ownership correction and retests

[The correction](18-native-child-stop.md) tracks live UID/start identities across
process groups and freezes ancestry to a stable closure before forceful Stop.
It excludes unrelated native daemons and never signals saved uncertain PIDs.
The former ordinary native separate-group escape is covered; deliberate
double-fork/reparent escape between observations remains outside the proof.
The existing bounded PTY tail and independent durable-history boundary remain.

The four real dashboard/native retests used fresh per-case empty Git checkouts
with private explicit state. Each reused existing authentication, accepted normal
folder/passive-hook trust, requested exactly the bounded heartbeat script and
selected one-time native command approval. Discovery preserved Git status.
A child with a separate PGID was observed before the real dashboard `s` or `q`.
All known heartbeat PIDs then disappeared, ledgers became stable, process state
was Stopped with execution null, pending capture zero, and written equaled durable.
The Stop cases also had owner-lifetime accepted equal durable. After ordinary
quit, that owner counter resets to zero on the new supervisor; it is not compared
with the retained session prefix. Both ordinary q cases restored exact termios.
Every verification tail obtained readable public History and confirmed Shutdown.

| Actual native action | Native PID | Ordinary tool PID / PGID | Heartbeat before→after then stable | Durable bytes |
| --- | ---: | --- | --- | ---: |
| Claude318 Stop |9414|Python10892 / shell-group10891|4→4|10315|
| Claude318 ordinary q |18528|Python21335 / shell-group21334|4→4|11355|
| Codex167 ordinary q |26362|Python30909 / own-group30909|4→5|28085|
| Codex167 Stop |67136|Python71254 / own-group71254|4→4|28018|

[Exact task/session/request/native IDs and binary hashes](18-native-stop-quit-summary.json)
are separate from the [raw observations, continuous whole-PTY streams, cells and
drivers](18-native-stop-quit.tar.gz), with [file/archive checksums](18-native-stop-quit-manifest.json).
Each captures source `a3842906fddfc5649af11a898f987785dc0e7ece` before launch.
Claude Stop used binary SHA256
`7006a3378227a95e489630347d378026e79b905fc6a90aedd536a67526fcefeb`;
the other three used `f2b0111750f50521f0c3bb0c1e4dc87d6fef963527916551ddc2584991e1553b`.
Cargo's all-target build relinked the same source artifact between launches.
The receipts preserve each actual prelaunch hash; no mapped runtime is silently
reassigned to the newly linked path. These are functional proofs, not latency
samples: collector waits and concurrent checks are included in coarse timings.

Three first collectors reached the correct native outcome but then asserted
incorrect bookkeeping: immediate Shutdown was provisional and needed polling,
and a restarted owner's accepted counter cannot equal a retained durable prefix.
Their original failed-collector receipts remain unchanged. Read-only completion
tails checked the same identities/PIDs, stable heartbeats, readable History,
written/durable extents and eventual stopped=true. One Codex Stop collector
aborted *before Stop* when its supervisor's transient inventory `ps` child made
a single-child assertion invalid; it is not counted. It was cleaned through
authenticated Shutdown. The corrected fresh Codex Stop selected only the known
native child under its verified proof owner and completed end to end.

[Foreground native working dashboard](screens/18-codex-owned-tool-running.png)
and [Stopped dashboard](screens/18-codex-owned-tool-stopped.png) are continuous
whole160×40 captures, visually inspected. After Stop the native pane intentionally
retains its last screen; the Process/semantic labels identify Stopped/Unknown,
so retained native “Working” prose is not claimed as current semantic evidence.
