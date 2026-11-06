# Milestone 2 ticket 15: external inspection and live source

Environment: macOS 26.2 (25C56), arm64 Mac16,8; stable Rust 1.99.1 / Cargo
1.99.1. Isolated worktree `/tmp/rat-ticket15`, branch
`ticket15-external-editor`, started at integration commit
`ea7deeafd92eb234805141c18c3f6131c15a491a`. All fixtures use disposable Git
repositories and explicit private state directories. No user repository or native
coding-agent conversation was edited. The independent coding child in the editor
proof is the existing scripted PTY, not a Claude/Codex compatibility claim.

## Working behavior

`e` from Working or Whole workspace requests fresh execution-owned full-comparison
content. It includes the inventory and selected comparison identity, capture time,
HEAD and applicable base/common ancestor. New, staged, unstaged, deleted, binary,
conflict, submodule and omitted/oversized content retain explicit states/reasons.
The file limit is 1024, display path labels are abbreviated at 256 bytes, per-file
content is 64 KiB and total export is bounded to 1 MiB. A five-second between-file
budget stops further content collection; bounded Git commands already running can
finish after that budget. Cancel/Stop and shutdown retain worker cleanup duties.
The owner transports content; the dashboard preparation worker writes a unique
private read-only artifact outside the checkout. Artifacts stay stable while open;
reopening captures a new request and new content.

`o` requests the existing selected live source file; `E` requests the existing
workspace directory. The owner validates retained checkout identity, rejects
missing files/path traversal and files resolving outside the checkout, and returns
an attributed explicit local edit location. Paths retain Unix bytes, and editor
executable/arguments/paths are separate argv operands. Vim positions the source at
the new-side hunk nearest the visible patch line. LIVE source labels distinguish
saves shared with the active agent from inspection snapshots.

Configuration uses `RAT_EDITOR_JSON`, with default Vim and optional literal
arguments and `vim`, `plus`, or `none` position syntax. GUI editor configurations
need their documented wait option. Main releases TerminalGuard ownership, then
polls a dedicated editor worker's spawn/wait completion without drawing or reading
input. The editor receives its own foreground process group. Main reclaims the
dashboard group and terminal modes and invalidates fullscreen buffers at the
actual current geometry. App project/workspace/session selection, input focus,
file/scroll/horizontal and history context remain in memory throughout handoff.

## Actual macOS editors

- `/usr/bin/vim`: Vim 9.1, macOS arm64, patches 1–1752; compiled November 8, 2025.
  Config: executable `/usr/bin/vim`, arguments `["-u", "NONE"]`, position `vim`.
  Inspection adds `-R -n -i NONE +<selected-artifact-line> -- <absolute-artifact>`;
  source adds `+<source-line> -- <absolute-source>`.
- `/usr/bin/pico`: Pico 5.09 (`/usr/bin/nano` symlinks to Pico on this machine).
  Version obtained with `pico -version`. Config: executable `/usr/bin/pico`,
  arguments `[]`, position `plus`; argv appends `+<line> <absolute-target>`.

`macos_vim_and_pico_inspection_source_save_resize_and_supervisor_continuity`
passed in 8.26 seconds, serially in the reserved terminal test window. For each
actual editor it opened Working and Whole workspace, changed source while
inspection was open, resized 160×40 → 100×30 → 160×40, returned to the same diff,
saved `editor-saved` into actual source, refused subsequently deleted source,
and reattached to the same surviving scripted session for an echoed input marker.
The test sent actual Ctrl-C bytes in both editors. While each editor was open,
`tcgetpgrp` identified a positive foreground group different from the dashboard;
after return, it identified the dashboard group again. Raw terminal attributes
matched their pre-handoff values. Vim `:cq` produced actual exit status 1 and
returned a usable dashboard. Pico's Ctrl-C retained its native cursor-position
behavior; Vim retained its native interruption behavior.

The standard dashboard PTY regression
`configured_editor_missing_executable_and_nonzero_return_keep_diff_context`
passed in 2.82 seconds: nonexistent `/missing/rat-editor` and configured
`/usr/bin/false` both restored exact raw attributes, the selected comparison and
`+current` patch, then quit successfully. The native Vim `:cq` check supplements
the generic nonzero configured-program case. These checks establish return for
spawn errors and nonzero exits; they do not claim a special Pico error-exit key.

## Checks and initial failures

- Build, formatting and Clippy (`--locked --all-targets -- -D warnings`) passed.
- Editor unit/real-Git checks: 3 passed, including literal argv with shell-like
  characters/newline, stable artifact and fresh reopening, deleted source,
  outside-checkout symlink/traversal refusal, attributed errors and retained
  dashboard/history/diff context, and source hunk positions.
- Working-diff checks: 11 passed. The added export test checks all-file Working
  and Whole content, exact revision labels, overlapping staged/unstaged edits,
  new/deleted/binary/oversized content and total bound using a real repository.
- App checks: 22 passed. UI checks: 13 passed, including normal/small geometry,
  retained selection, focus and native screen layout.
- Existing real dashboard Whole normal/small PTY regression: passed in 4.30s.
  Reference composition remains Reviews/tree/selection on the left and the
  prominent active comparison/session on the right; compact layout retains the
  comparison and keyboard actions. Added controls occupy help text rather than
  changing pane composition. No inline source editor was introduced.
- Existing normal/error/panic terminal restoration PTY test: passed in 0.01s.

The first failure-return PTY attempt exposed a product bug: calling Ratatui
`Terminal::clear()` after handoff queried cursor position and timed out on the
outer PTY. Fullscreen `resize()` now resets buffers without that unnecessary
query; the failure-return check then passed. An initial native proof attempt
sent `e` as soon as the Whole title appeared, before its base comparison finished
loading. The harness now waits for the returned patch before invoking the editor;
the first complete native run passed in 7.70s, followed by the enhanced 8.26s run.

A test attempted an invalid UTF-8 filename; this host's filesystem rejected it
with `EILSEQ` before application code ran. The test now uses supported literal
Unix path bytes containing newline and shell metacharacters. No unsupported
filesystem filename capability is claimed. Likewise `nano --version` was found
to enter Apple's Pico rather than print a version; that owned plain-pipe process
was immediately terminated. Subsequent qualification used Pico's actual version
flag and an explicit terminal test slot.

After the native window, process inspection found no owned editor, dashboard,
supervisor or scripted-child executable left running. The terminal slot was
explicitly returned to the coordination compatibility worker.

## Source boundary after native qualification

The native run's `target/debug/agent-terminal` SHA-256 was
`666c939a5cc69b60a1fbf5afccb4a075ca48b49aca52cdfcc9dd4d52f5f71dfc`.
It used the ticket-15 worktree source based on `ea7deea`; the TerminalGuard,
foreground process-group assignment, worker spawn/wait and main's suppressed
input/render handoff passed the native cases above. Their process/terminal
handling is unchanged after that passing native run.

Later state-only hardening added explicit `EditorContext` preservation and
processing of an intervening session observation before restoring navigation.
The filtered App regression preserves a real selected HistoryPage passage at
byte offset 16384, historical session/index, raw mode, scroll 29, project,
workspace and owner, plus selected diff file/scroll 13/horizontal 24. A changed
history/output observation is applied without losing that retained passage.
Native focus is restored only for the same still-running controllable session;
an exited or replacement session retains dashboard focus. That regression and
all 22 App tests passed. Reopening/stable-artifact and literal-argv real-Git
coverage was also extended and passed. Compact help now exposes editor actions.
These later state/test/help additions were not relabelled as a final-source
native rerun; complete final-source editor revalidation remains with ticket 19.
The broad all-target/native suite was serialized under the separate coordination
compatibility owner; this ticket ran the scoped checks recorded above.

## Limits

This qualifies the recorded local editors/configurations and disposable workload.
It does not qualify GUI editors, remote source editing, editor suspension via
job-control shortcuts, arbitrary personal editor startup scripts, or the combined
multi-project/native-agent/reboot workload reserved for ticket 19. External file
removal/replacement after owner validation can race the editor's own open; missing
locations are never intentionally substituted with invented editable files.
