# Milestone 2 ticket 07: first prompts and local workspace names

Actual macOS qualification, 2026-11-06: arm64, macOS 26.2 (25C56), stable
Rust/Cargo 1.99.1, debug locked build. Final native runs used clean runtime
`1be74b28fe3f14706cb0a1f1a904cdf4290eaa24`, binary SHA256
`09148c6d03bb47f88e624eeb349ba235d092ead34d2b47642b38d68ef07ad021`.
The later test/documentation commits and merge of integration `7cccc21` have
no difference in `src/`, Cargo.toml or Cargo.lock from that runtime.

## Working behavior

`C` and `X` open an optional first-prompt editor for the adopted workspace.
Enter explicitly starts the selected native tool; Esc cancels. Existing `c` and
`x` start without an initial prompt. Native interactive argv receives the
supplied prompt as one argument following `--`, after the existing status-hook
configuration. There is no injected naming turn, print/SDK replacement, hidden
provider or recorded raw-key stream. Native trust, hook review, input and
approvals remain native interaction. Prompt input is limited to 4084 bytes;
NUL is refused. Resume receives no initial prompt.

New workspace records get stable `Workspace <first eight ID characters>` labels.
A first supplied prompt replaces only that fallback with a local readable label:
control/whitespace collapse, first 72 Unicode characters plus an ellipsis when
longer. This is explicitly workspace metadata, not a native conversation title.
`R` edits the selected workspace name, Enter saves durably, Ctrl-U clears, and
Esc cancels. Manual names and labels from prior prompts survive future prompt
launches. Names from older registry versions remain retained rather than being
silently re-derived. Schema 5 records fallback/prompt/manual/retained provenance
in a separate small table; naming does not change workspace/task IDs, canonical
checkout identity, folder, branch, session membership or native conversation ID.

The sole persistence worker applies a prompt-derived default atomically with
launch preparation. Existing prepare/authorize/exec-receipt/result boundaries
still govern execution. The prompt travels only through the live pending and
creating request; restoration does not replay it. Stop cancels pending launch
intent, including a prepare blocked on storage. A late completion cannot start
that cancelled request. Same-request launch/query recovers the retained result
without submitting a second prompt. Manual renaming alone cannot start coding.
The additive protocol capability is checked before using a live owner; migration
still requires the existing sole-owner lock.

## Actual native results

Installed `claude --help` states `claude [options] [command] [prompt]` and an
interactive default. Installed `codex --help` states `codex [OPTIONS] [PROMPT]`
and describes the optional initial prompt. These installed help outputs, rather
than an assumed remote API, selected the invocation. Canonical paths, native
binary hashes, actual `ps` argv and `lsof` cwd are in the
[summary](m2-07-native-summary.json).

| Native tool | Final run | Native PID | Written = durable at Stop | Result |
| --- | --- | ---: | ---: | --- |
| Claude Code 2.1.319 | `native-claude-three` | 26055 | 14266 bytes | passed |
| Codex CLI 0.167.0, `--no-daemon` | `native-codex-one` | 30160 | 39674 bytes | passed |

Each ran in its own fresh `/private/tmp/rat07-<run>-checkout` Git repository and
explicit private `-state` directory. Existing native authentication was reused;
no authentication secrets or passwords were collected. Native tools maintained
their ordinary conversation/trust records. No user/default supervisor was used.
The complete exact first and follow-up prompts are recorded as declared workload,
not a stream of pressed keys.

Both first prompts requested the exact `PROMPT07-CLAUDE` / `PROMPT07-CODEX`
marker and a question about running a bounded heartbeat, without tools. Actual
native answers showed the marker and question. The prompt-derived local name
was observed before any rename. Detach, protocol inspection and dashboard reopen
preserved the same managed/native conversation. An explicit follow-up answered
the question and requested only `python3 proof-heartbeat.py`, a proof-owned
bounded foreground command. Native one-time approval was observed and accepted;
no persistent command allow or permission bypass was selected. Heartbeats proved
execution in the isolated checkout. Native Ctrl-C was sent while that command
was active. The actual R editor then saved `Manual claude workspace` /
`Manual codex workspace` with unchanged workspace/folder/session/native IDs.
A QueryLaunch of the original UUID returned the same identity.

Stop settled to Stopped, execution null, pending capture 0 and written=durable.
Authenticated native and heartbeat PIDs were absent afterward. Shutdown was
polled to confirmed stopped=true. Restart retained the exact manual name,
managed/native IDs and stopped state with no execution; final Shutdown was also
confirmed. The dashboard restored its outer terminal modes after detach. The
final process audit found no driver-owned dashboard, supervisor, native CLI or
heartbeat left. Native Ctrl-C may itself finish a foreground command before
Stop; this is scoped evidence for the combined native-interrupt/managed-stop
path, not a new independent descendant-stop matrix.

## Artifacts and failed attempts

[Verified archive](m2-07-native-evidence.tar.gz) contains exact executed drivers,
continuous outer-PTY output, parsed cells, run observations, normal/small editor
captures and the failed attempts. [Manifest](m2-07-native-manifest.json) gives
SHA256 for every entry and the archive; every archived entry was reopened and
verified. The original first failed driver was reconstructed and verified against
its recorded prelaunch SHA256 before inclusion.

Claude attempt `one` successfully delivered the first prompt, native question,
one-time approval, foreground command and interrupt, then the driver hit a
Python bytes/string concatenation error before typing the manual name. Its
fallback cleanup was blocked by the still-open modal. The owned dashboard was
terminated, and an explicit private-endpoint Stop/Shutdown then confirmed
Stopped, pending 0 and shutdown true; its native/supervisor/dashboard PIDs were
absent. Its screenshot also exposed stale native cells under the text editor;
`1be74b2` clears that area before rendering the editor.

Claude attempt `two` passed the initial answer/naming stage, then reopened the
dashboard before the previous controller EOF had been released. The dashboard
honestly refused competing control. The driver timed out, then explicitly
stopped the native session. The final driver waits/retries that existing
connection boundary; the product's single-controller rule was preserved.
These failures are not counted as passing final runs.

## Verification and limits

Focused checks passed: both first-prompt/name and stalled-prepare cancellation
ownership scenarios; three registry migration/re-adoption scenarios; project
registration/failure and normal/small keyboard scenarios; and the new real-PTY
prompt-editor/manual-rename scenario at 160×40 and 60×18. The last scenario
cancels its prompt and saves its name with no native session identity. App's
explicit-submit/cancel test and eleven UI rendering tests passed. Locked build,
formatting and all-target Clippy with warnings denied passed.

The first new test initially hung in the harness while waiting for an undrained
outer PTY; its own test process was terminated and its private supervisor was
explicitly shut down. It now drains through the existing dashboard kill seam.
Normal/small registration also caught C/X/R stealing uppercase characters from
project-path input; modal precedence was corrected before both final native runs.
An immediate test reconnect race now uses the existing refusal/retry behavior.
The complete all-target suite has not been re-run on this branch: the preceding
combined integration gate records its separate missing-checkout resume-display
failure in [its report](m2-prerequisite-integration.md). This ticket does not turn
that earlier gate into a passing full-suite result; final combined integration
must run the full required suite.

The [normal rename](m2-07-rename-editor-160.png) and
[small prompt](m2-07-prompt-editor-60.png) images rasterize real parsed dashboard
text. Normal layout retains reviews above the project/worktree tree beside the
main pane, as in the vision; the explicit editor temporarily occupies that pane.
Small layout hides the sidebar and retains visible submit/cancel controls. Native
captures in the archive preserve full cells/styles. These are not Terminal.app
or iTerm screenshots and do not establish those graphical terminal integrations.

No dependable native conversation-title ownership was established or used.
Missing prompts receive the stable fallback, and legacy labels stay retained.
Claude 2.1.319 launch/input/naming is qualified here; its semantic status remains
honestly Unknown when the existing version-qualified adapter lacks fresh evidence.
This does not requalify the complete earlier semantic-status/native-resume matrix.
Readable/searchable-history projection is ticket 08. This slice operates on
adopted workspaces; creation/setup and their Create without starting control are
subsequent tickets. Registration/adoption/startup/restore/reconciliation do not
constitute launch intent, and none of these observations submits a prompt.
