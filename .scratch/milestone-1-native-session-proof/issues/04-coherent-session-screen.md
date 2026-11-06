# 04: Keep the session screen coherent

Status: ready-for-agent
Blocked by: 03
Resolved: yes

**What to build:** Resize and leave or re-enter a session while output continues, with a coherent screen and no lost or duplicated captured output.

- [x] Resize the PTY and screen parser in order to the actual session viewport, including sidebar and border allocation; do not use the full outer terminal dimensions.
- [x] Attaching synchronizes a screen snapshot with subsequent output at a consistent boundary. The same running child remains attached across repeated focus changes.
- [x] The child continues to be drained in dashboard focus and when no dashboard is attached, subject to declared bounded buffers. Screen/scrollback remains bounded.
- [x] The fixture exercises repeated attached and detached resize, output during reattach, split escape/UTF-8 sequences, redraws, alternate-screen programs and mouse-mode requests. Verify child dimensions, coherent visible contents and captured-output continuity.
- [x] Normal and smaller terminal rendering keeps session identity, focus, status and essential keyboard actions accessible. Tiny sizes must remain safe; record any required layout compromise.
- [x] Record parser/terminal feature limitations and screenshot differences from the reference image. Raw child controls must not change the real dashboard terminal outside its owned modes.

## Comments

Implemented viewport-derived PTY/parser resizing, coherent styled snapshots with
parsed-output boundaries, bounded detached screen draining, cell/cursor rendering
and compact/tiny handling. The real dashboard/supervisor/scripted-child proof
passes alongside locked build, fmt, warnings-denied Clippy and all-target tests.
See [evidence and visual comparison](../../../docs/proofs/04-session-screen.md).
Continuity here is ephemeral parsed output; durable transcript/history remains
ticket 08. Native Claude/Codex support and sustained resource measurements are
not claimed.
