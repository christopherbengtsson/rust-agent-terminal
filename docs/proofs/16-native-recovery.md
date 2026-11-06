# Ticket 16: explicit native recovery, independent readable history

2026-11-06, macOS 26.2 (25C56), arm64. Real dashboard and supervisor processes,
Python 3.14.3 macOS PTYs at 160×40 with a 104×32 child pane, TERM
`xterm-256color`, stable Rust/cargo 1.99.1, portable-pty 0.9.0, vt100 0.18.1,
Crossterm 0.29.0 and Ratatui 0.30.2. Parsed text captures establish these PTY
scenarios; no graphical terminal application/version or physical reboot is claimed.
Socket/native operations ran outside the execution sandbox after its Unix-socket
restriction prevented the first isolated test's supervisor startup.

## Actual original-conversation recovery

| Native tool | Exact binary SHA-256 | Native conversation | Original → resumed PID |
| --- | --- | --- | --- |
| Claude Code 2.1.318, Haiku 5.5 medium | `677a2ea7bf576cedd20e191d973914ed8a41796f53a63b1923381ddc4d5b5e7d` | `4d99aebe-62cb-49bb-993d-8819bb13edc2` | 27657 → 30763 |
| Codex CLI 0.167.0, GPT-6.1-Sol medium | `030316ecfbb41f4e7946e584b0063ef333b7a36cd312ba349f2f6869e39b62cc` | `01a1b119-bc22-7680-9457-78fe79526d35` | 29668 → 34108 |

Claude was pinned by a proof-only PATH symlink to the existing canonical
`~/.local/share/claude/versions/2.1.318`. The installed default symlink's newer
2.1.319 is not this claim. Codex resolved to
`~/.codex/packages/standalone/releases/0.167.0-aarch64-apple-darwin/bin/codex`.
Local `--help`/`resume --help` was checked before execution: Claude accepts an
explicit `--resume <id>`, while Codex accepts `resume [SESSION_ID] [PROMPT]` and
`--no-daemon`. The [Claude CLI reference](https://code.claude.com/docs/en/cli-reference)
and [OpenAI command reference](https://learn.chatgpt.com/docs/developer-commands?surface=cli)
corroborate the mechanisms. No `--last`, `--continue`, picker, fork or follow-up
prompt selected the original conversation on the product's behalf.

For each tool, the real dashboard adopted a fresh empty Git checkout:
`/private/tmp/rat16-claude-checkout` or `/private/tmp/rat16-codex-checkout`.
Private supervisor state was `/private/tmp/r16c` or `/private/tmp/r16x`.
Existing native authentication was reused. Native folder trust was accepted for
these empty checkouts. Codex's ten passive invocation-local monitor hooks were
reviewed and trusted in its native Hooks UI, including the changed generation
at resume. No credentials were copied, no global/repository configuration was
edited, and no bypass flag was used. Native session/trust records remain owned
by the native tools.

1. Launch through dashboard `a`, then `c` or `x`. Claude gets its generated
   UUID through `--session-id`; Codex generates its own UUID, captured from the
   invocation-local SessionStart receipt. The supervisor saves only that native
   identity alongside the existing tool, task, managed session, launch request
   and checkout metadata.
2. Through the attached native pane submit: `Remember this conversation-only
   token: <token>. Reply exactly STORED. Do not use tools, write files, or repeat
   the token.` Claude's token was `amber-orbit-16-4f72`, Codex's was
   `violet-comet-16-b831`. Both answered `STORED`.
3. Kill only the isolated dashboard, reopen it, and observe the same native ID,
   original managed session and original native PID. This is live reconnect,
   independently of native resume. The first exploratory Codex harness invocation
   accidentally reused the Claude state directory, therefore never launched
   Codex; it was excluded. The recorded Codex run used its separate state above.
4. Enter native `/exit` through the dashboard. Both tools displayed the exact
   resume ID listed above. The supervisor reported Exited, execution reservation
   cleared, metadata committed, and history retained. Kill the now-idle isolated
   supervisor; reopen and observe the same exited record without coding startup.
5. Explicitly press dashboard `v`. It launches Claude `--resume <original-id>`
   without a conflicting new `--session-id`, or Codex `resume <original-id>
   --no-daemon`, with no supplied follow-up. Native conversation replay appears;
   no model turn is started until a user submits input. Each new runtime gets a
   new managed session/request and history segment, retaining the same task and
   native conversation identity.
6. Ask through the attached pane: `What exact conversation-only token did I ask
   you to remember earlier? Reply with that token only. Do not use tools or read
   files.` The prompt does **not** contain the token. Claude answered
   `amber-orbit-16-4f72`; Codex answered `violet-comet-16-b831`.
   These new model responses establish actual conversation continuation beyond
   replaying a saved terminal screen. [Claude capture](screens/16-claude-resumed.txt),
   [Codex capture](screens/16-codex-resumed.txt).
7. Exit both tools natively again. Temporarily move each checkout aside. Dashboard
   `v` reports `Checkout unavailable: cannot continue in place; h opens retained
   history. Restore the checkout before explicitly resuming.` No execution starts.
   Open `h`, select the original managed segment with `n` when needed, and page
   with `]`. The original marker remains readable with terminal controls escaped:
   Claude raw offset 2048, Codex offset 16384. The saved page identities are the
   **original** managed sessions, independent of both resumed processes and the
   missing checkouts. [Refusal and retained pages](screens/16-independent-history.json).
8. Restore the temporary checkout paths, explicitly shut down both isolated
   supervisors, and observe `stopped=true`. `git status --porcelain` returned empty
   for both. No proof coding child was intentionally left running.

Original/resumed managed session IDs respectively:
Claude `0a786524-0056-4a7a-86cf-50d5d068eabd` /
`23624514-2930-49c7-8732-020364d4b2ac`, task
`71af9058-94ec-4c41-9a2f-31f15b331b91`;
Codex `05222c8d-84e4-4bb9-8029-5efda0b85225` /
`7d82f17e-bd0c-4fb7-bccb-e21889253942`, task
`d35710c7-a58e-4b66-9104-73b2fc1ee952`.

## Safety boundary and fixture evidence

`v` is a distinct launch intent and goes through the same durable prepare,
process gate, authorization, result cache and exclusivity restrictions as new
conversations. Reconnect/startup/reconciliation never issues resume or a model
prompt. A current child refuses a second execution. A restored uncertain native
execution blocks both resume and new conversation until ticket 09's evidence
policy releases it. Same-boot leader absence cannot exclude escaped descendants;
this proof does not weaken that gate or claim a physical reboot.

The approved real dashboard/supervisor/scripted PTY seam and existing App/render
seams were used with real temporary Git repositories. The first App test failed
because Resume did not exist. The native-style Codex test then failed because
its real SessionStart helper receipt was not saved (`native_conversation=null`).
After implementation it persisted the known UUID across native-style exit and
supervisor restart; dashboard `v` delivered that original ID with `--no-daemon`
and no follow-up, while the previous segment remained readable.

The existing survivor test now also refuses `Resume` during same-boot uncertainty.
A separate **injected prior-boot fixture**, with no surviving process, restores a
Running Claude record and original UUID as Interrupted. It starts no child;
resume before reconciliation is refused; successful `u` still starts no child;
only explicit `v` passes the original `--resume <id>` to the native-style fixture.
This establishes reboot-style application behavior, not an actual OS reboot or
real native model recovery after an active supervisor crash. Actual native
conversation recovery was demonstrated after normal exit, separately above.

Unavailable recovery (a scripted child has no native conversation identity) gives
an explicit reason and leaves readable history. `c`/`x` are explicitly labeled
new conversation choices; there is no silent fallback from failed resume. Missing
checkout behavior also runs through the real dashboard fixture test. History
search, archival, arbitrary historical-session selection for resume, importing
external sessions, cross-tool context transfer and a native-record availability
preflight remain outside this proof. A native tool can reject a saved ID or newer
version; its native error remains captured/readable, without an automatic new
conversation. This narrow path resumes only the most recent managed native
conversation in the current adopted task.

The native recovery implementation was exercised before the final independent
status/subscriber merges. Final integration retains its flags and identities and
is checked at the scripted native-style PTY seam. Claude semantic-status gates
remain separately open; successful resume does not establish them. The dashboard
keeps the existing native pane and one-active-session layout, with `v resume`
and explicit new-conversation choices in normal/small keyboard help.

## Final verification and remaining combined check

After merging integration `6da639d` (final Codex status, bounded subscriber
frames, owned-PTY cleanup and error-first disconnect diagnostics), locked build,
format check and warnings-denied all-target Clippy passed. `cargo test --locked
--bin agent-terminal` passed 22 tests; the one ignored terminal subprocess probe
was exercised by the passing restoration driver. The native-filter ownership
run passed all eight scenarios in 15.66s, including original-ID persistence,
explicit resume, prior-boot restoration, unavailable recovery and survivor
exclusion. The unchanged unsafe-private-directory check passed in 0.81s.

An initial pre-ticket-13 aggregate run passed 27/28 ownership scenarios; its
bounded-history read exposed a truncated socket frame under the old write-timeout
path. Ticket 13's retained partial-frame cursor corrected that failure.

Two later full `cargo test --locked --all-targets` attempts each passed 31/32
ownership scenarios, including bounded-history continuity and the sustained
capture workload. The first failed because a disconnected diagnostic wrapped
an exact screen-string assertion; ticket 13's error-first display fixed it,
without changing the assertion. The second exposed a **harness ownership race**:
a new custom-PATH dashboard arrived before the supervisor consumed the prior raw
controller's EOF. Its actual screen said `Another dashboard controls this
supervisor`; no native-ID mismatch occurred. Eight focused repetitions had
passed, illustrating why pass counts alone did not diagnose that race.

The correction reuses ticket 07's bounded retry on that explicit public refusal,
now with a custom-PATH parameter. The native recovery scenario deliberately opens
a second dashboard while the raw controller is still held, observes refusal,
then closes the controller and reacquires through that helper. UUID equality,
metadata-commit acknowledgement and original-history assertions remain intact;
there is no arbitrary release sleep. The corrected scenario passed alone in
3.31s and within the final eight-scenario run. The prior-boot fixture uses the
same release helper. No production recovery policy was relaxed.

A final combined all-target run after this harness-only correction is left to
the integration merger, avoiding another duplicate multi-minute resource
workload. Earlier failed aggregate runs are not represented as passing that
combined check. No physical reboot, active real-native supervisor-crash recovery,
other native versions or full resume-management workflow is established here.

The combined check subsequently passed on `be1b40b`, integrated by `e2cfef3`:
22 unit tests, 33 ownership/history/recovery scenarios (197.55 s), scripted
interaction/status (14.50 s), and screen integration (8.18 s). The ignored
restoration probe was exercised by its parent test. Locked build, formatting,
and all-target warnings-denied Clippy passed. This includes the corrected
controller-release scenario, partial-frame delivery, and bounded OSC parser.
See [the combined verification record](13-slow-subscribers.md). Later storage
fault and final milestone checks remain separate.
