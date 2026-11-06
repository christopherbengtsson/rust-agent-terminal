# Ticket 03: native input and focus fixture proof

2026-11-06; macOS 26.2 (25C56), stable rustc 1.99.1, portable-pty 0.9,
Crossterm 0.29, vt100 0.18. Automated native macOS PTYs, with a real dashboard,
private supervisor socket and scripted child in a temporary real Git checkout.
No Claude Code or Codex CLI interaction is claimed; their compatibility gates,
including final reserved-chord validation, remain open.

## Observed behavior

The preimplementation application test failed on Alt-x (`78` instead of `1b78`)
and passed after bounded legacy key encoding. The dashboard modifier test failed
because Ctrl-a adopted a checkout; plain dashboard actions now require the
appropriate unmodified input. State tests verify Ctrl-C is input, Ctrl-]/Ctrl-5
return focus without shutdown, Enter reattaches to the same observed session,
paste is ignored in dashboard focus, and oversized paste produces a retryable
error. Rendering tests show focus and available actions, including interrupt and
escape while attached, and quit only in dashboard focus.

The end-to-end scripted test sends these bytes through the outer PTY, terminal
event parser, App and supervisor and checks the child's byte receipts:

| Input | Child receipt (hex) |
| --- | --- |
| λ, split across outer writes | `cebb` |
| Bracketed multiline paste `alpha\nβ` | `1b5b3230307e616c7068610aceb21b5b3230317e` |
| Alt-x, Ctrl-Up, F1, Shift-Tab | `1b781b5b313b35411b4f501b5b5a` |
| Up in requested cursor application mode | `1b4f41` |
| Ctrl-C | `03`, fixture prints `interrupted: 03`, dashboard stays attached |

Unbracketed multiline paste is separately checked as two native Enter-delimited
lines after the fixture disables bracketed paste.

The fixture splits UTF-8 and the bracketed-paste mode escape across child writes;
the parsed screen remains readable. `approve` displays a native-style y/n prompt;
`y` produces `approval: yes`. Raw `1d` returns focus to the dashboard, Enter
reattaches, and `identity` confirms the same child PID. Discovery leaves the
checkout unchanged. Attached keys never invoke dashboard navigation or quit.
The fixture is a raw-mode native-style program: Ctrl-C behavior here proves
byte delivery and its explicit interrupt handler, not a native agent's lifecycle
or a canonical-mode process group's SIGINT semantics.

A separate subprocess PTY probe enters the real terminal guard and exits
normally, returns an error, or panics. For each scenario it compares complete
termios before/after and observes alternate-screen enter/leave plus bracketed-
paste enable/disable sequences. The test first failed for missing paste mode;
after enabling and restoring it all three paths pass. It uses a real terminal
rather than mocking Crossterm. No terminal emulator application is inferred.

## Limits

Legacy xterm UTF-8, control characters, Alt prefix, arrows/navigation, F1–F12 and
Shift-Tab are encoded. Key encoding runs in the supervisor and uses the current parser cursor-key mode. Shift/Alt/Ctrl cursor and function modifiers use xterm's
modifier parameter. Super/Hyper/Meta, unsupported keys and control characters
without a legacy encoding produce visible errors. Legacy terminal ambiguities
remain; enhanced keyboard protocols and
mouse forwarding are not implemented. Key releases are ignored; repeats remain
native input. Unicode control shortcuts never truncate a Unicode scalar to a
control byte.

Paste wrapping is performed outside the UI loop in the supervisor according to
the current child parser mode. Unbracketed paste converts CRLF/LF to CR. A paste
over 4084 UTF-8 bytes is rejected before dispatch, preserving the 4096-byte framed
input cap including delimiters. No automatic chunking risks partially queued
paste. Crossterm buffers the full incoming paste event before this limit can be
checked. Bounded command-queue saturation remains an explicit error requiring
retry, not a delivery guarantee. Arbitrary unsupported terminals/IME behavior and
real native tools remain unverified. Ctrl-] also reserves legacy Ctrl-5; Alt-Ctrl-]
is forwarded rather than treated as the reserved chord.

## Validation

The isolated Unix socket requires an escalated test run in this sandbox. A
sandbox connection failure was distinguished from behavior; the same test outside
the sandbox established the intended red paste failure, then passed after the
child requested bracketed paste. Final stable locked build, formatting, Clippy
and all-target tests are recorded with the ticket completion comment.
