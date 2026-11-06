# 04: Keep the session screen coherent

Status: ready-for-agent
Blocked by: 03

**What to build:** Resize and leave or re-enter a session while output continues, with a coherent screen and no lost or duplicated captured output.

- [ ] Resize the PTY and screen parser in order to the actual session viewport, including sidebar and border allocation; do not use the full outer terminal dimensions.
- [ ] Attaching synchronizes a screen snapshot with subsequent output at a consistent boundary. The same running child remains attached across repeated focus changes.
- [ ] The child continues to be drained in dashboard focus and when no dashboard is attached, subject to declared bounded buffers. Screen/scrollback remains bounded.
- [ ] The fixture exercises repeated attached and detached resize, output during reattach, split escape/UTF-8 sequences, redraws, alternate-screen programs and mouse-mode requests. Verify child dimensions, coherent visible contents and captured-output continuity.
- [ ] Normal and smaller terminal rendering keeps session identity, focus, status and essential keyboard actions accessible. Tiny sizes must remain safe; record any required layout compromise.
- [ ] Record parser/terminal feature limitations and screenshot differences from the reference image. Raw child controls must not change the real dashboard terminal outside its owned modes.
