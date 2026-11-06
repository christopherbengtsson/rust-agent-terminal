# Ticket 04: coherent session screen

2026-11-06; macOS 26.2 (25C56), arm64, stable rustc 1.99.1; debug build,
portable-pty 0.9.0, vt100 0.18.1, Ratatui 0.30.2. This is a scripted native-style
PTY proof, not Claude Code or Codex CLI compatibility evidence.

## Reproduce and results

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets -- --nocapture
RAT_SCREEN_EVIDENCE="$PWD/docs/proofs/screens" cargo test --locked --test session_screen
xcrun swift scripts/render-terminal-capture.swift docs/proofs/screens/04-normal.json
xcrun swift scripts/render-terminal-capture.swift docs/proofs/screens/04-small.json
```

All four required checks passed. Raw final test summaries:

```text
unit: 14 passed; 0 failed; 1 ignored (subprocess probe driven by restoration test)
ownership: 4 passed; 0 failed; 1.65s
scripted_session: 1 passed; 0 failed; 2.08s
maximum 200x80 styled snapshot: 358069 encoded bytes
session_screen: 1 passed; 0 failed; 6.59s
```

Tests require an approved run outside the sandbox for isolated Unix sockets and
PTYs. The initial sandbox run failed to acquire its supervisor; the approved run
then established the expected red failure (`size` was echoed rather than answered).
The viewport/render test initially failed to compile for missing viewport geometry.
Both pass after implementation. The style extension exposed the harness's
inherited NO_COLOR environment: Crossterm emitted empty color resets, also clearing
bold. The screen proof explicitly uses TERM=xterm-256color and NO_COLOR empty;
it verifies emitted styles rather than inferring them from text. The user's own
NO_COLOR environment is respected by the product and can suppress native colors.
Test durations are not latency measurements or a sustained resource proof.

The primary seam drives a real dashboard, supervisor, and child inside temporary
real Git repositories and private state directories. It checks child-reported
104×32 geometry for a 160×40 terminal, 64×16 for 100×24, and 58×11 for 60×18.
Resizes occur in attached and dashboard focus. A styled clear/redraw splits UTF-8
λ and 界 and an escape sequence across writes. The outer PTY screen verifies
red/bold text, wide-cell continuation and the actual visible cursor position.
Alternate-screen entry replaces the primary screen; leaving restores it. Child
mouse requests do not enable mouse capture on the dashboard's terminal.

The fixture streams 200 numbered lines, every 5 ms, through focus changes and
reattachment. After dashboard death the public supervisor protocol observes the
same session identity and parsed-byte boundary. A second 200-line stream runs
with no dashboard connection; the next snapshot advances by exactly 2,809 bytes
(200 × 14-byte records + nine prompt bytes), with the final numbered line visible.
No new child is launched. This verifies continuous ephemeral parsing, not durable
transcript capture: ticket 08 still owns history. The same seam requests oversized
geometry, observes the 200×80 clamp, fills every cell with varying RGB styles,
and successfully receives the complete 358,069-byte snapshot. It then restores
small geometry and reconnects the dashboard to the same child.

## Screen and resource contract

One shared layout derives the inner viewport after sidebar, status/help and borders.
The supervisor holds the parser lock while resizing the PTY, then its parser;
output parsing cannot interleave those geometry operations. New children start at
the latest viewport. Every snapshot captures contents/styles/cursor/modes and the
processed-byte count under that lock. The transport worker reconstructs display
cells outside the UI loop. The UI draws only cells within its owned rectangle,
clips wide glyphs at the right border during resize, and translates the native
cursor into dashboard coordinates. It never emits the captured child escapes.

Caps: 200 columns × 80 rows per primary/alternate screen, zero scrollback, 4 KiB
PTY reads, 32 queued commands, one queued observation, and a 4 MiB protocol frame.
Snapshots are complete replacements, so a lagging UI resynchronizes instead of
accumulating incremental output. The reader continues when focus is elsewhere or
there is no client; there is no growing detached transcript buffer. Existing input
and paste caps remain 4,096 and 4,084 bytes. A hidden/zero-size inner viewport is
reported as at least 1×1 to the PTY/parser. Dimensions beyond the cap leave extra
pane space unused. These finite caps and the full-cell test are not measurements
of peak memory, sustained throughput, slow-subscriber fairness or queue pressure;
those remain tickets 13/17.

## Visual comparison and limitations

[Normal 160×40 capture](screens/04-normal.png),
[small 60×18 capture](screens/04-small.png). The adjacent JSON files retain parsed
PTY cells and cursor; the Swift script rasterizes those observations using Menlo.
They are terminal-cell captures, not screenshots of a particular terminal app;
font fallback, cursor shape and terminal chrome are illustrative.

The normal composition follows the reference image: reviews above the project /
worktree / session tree at left, large cyan-bordered session at right, dark
background and keyboard actions below. Unlike the illustrative reference, reviews
are honestly unavailable, there is one session in one adopted worktree, semantic
status is unknown, and the right pane renders a native terminal rather than a
message composer. The current status block consumes more rows and UUIDs consume
more space than the reference; a later identity presentation could reduce that
cost without hiding the current task/session.

Below 80 columns or 22 rows, the sidebar is hidden. The pane keeps a short session
identifier and focus, adopted path remains in status, and two help lines retain
focus/status plus essential actions. The screenshot shows the same session tag
as the normal capture. Full path/UUID text can still truncate, and project-tree
navigation is visually unavailable until the terminal grows. This explicit
compromise preserves native interaction space. At tiny sizes (12×4 tested),
rendering and resize remain safe but full metadata/actions cannot fit; enlarge the
terminal for usable operation.

vt100 supports incremental text, cursor motion, erase/redraw, basic SGR colors /
bold/dim/italic/underline/inverse, wide and combining characters, primary/alternate
screens and input-mode tracking. Rendering preserves those cell styles and cursor
visibility/position. Cell storage limits combining sequences to the parser's
fixed 22-byte cell representation. Reflow, full grapheme shaping, ligatures,
custom cursor shapes/blinking, hyperlinks, images/sixel, terminal-query replies,
enhanced keyboard protocols and mouse forwarding are not established here.
Unsupported child controls remain inside the parser and cannot alter dashboard
modes, titles or clipboard. Real native agents and terminal-emulator compatibility,
durable history and supervisor-crash reconciliation remain open proof gates.
