# Ticket 12: evidence-based Codex status

2026-11-06, macOS 26.2 (25C56), arm64; Codex CLI **0.167.0**, GPT-6.1-Sol
medium. Canonical executable:
`~/.codex/packages/standalone/releases/0.167.0-aarch64-apple-darwin/bin/codex`;
SHA-256 `030316ecfbb41f4e7946e584b0063ef333b7a36cd312ba349f2f6869e39b62cc`.
Rust/cargo 1.99.1, debug profile, portable-pty 0.9.0, vt100 0.18.1,
Crossterm 0.29.0, Ratatui 0.30.2, Python 3.14.3. Real macOS PTYs had
TERM=xterm-256color, outer geometry 160×40 and child pane 104×32. No graphical
terminal-emulator application was used or certified. Screens below are parsed
native PTY screens paired with the supervisor's semantic observation, not
app-server events or `codex exec --json` output.

## Result and validity

The five required signals passed for this version and the declared scenarios.
This is a bounded status compatibility result, not release-wide native support.
The dashboard's `support gate open` reason still refers to the wider experimental
integration; other milestone requirements and other CLI versions are separate.

| State | Required evidence and actual native scenario | Matching capture |
| --- | --- | --- |
| Working | Current main-thread turn's diagnostic-only PreModelRequest, until PostModelRequest or five seconds. A real model response was actively generating; process liveness alone was insufficient. | [Working](screens/12-codex-working.json) |
| Input needed | Open request_user_input PreToolUse ID in the submitted turn, plus the current numbered question modal, selected option and exact native answer footer. `/plan` enabled the real question tool; this was not a conversational question. | [Input](screens/12-codex-input-gated.json) |
| Approval needed | Unique open Bash tool ID and same-turn PermissionRequest, plus the current native command approval modal and exact confirm/cancel footer. PermissionRequest lacks a tool-use ID in this version, so ambiguous parallel matches stay unknown. | [Approval](screens/12-codex-approval-gated.json) |
| Turn completed | Current-turn Stop, followed by a later parsed output boundary with a new `Worked for … • HH:MM` marker and the native idle composer/footer. Stop alone is insufficient. | [Completed after continuation](screens/12-codex-completed-continuation.json) |
| Interrupted | Current main-thread turn's actual native Interrupt hook acknowledgement, not delivery of Ctrl-C. Native Conversation interrupted was visible and the process remained Running. | [Interrupted](screens/12-codex-interrupted.json) |

The invocation channel has a fresh token. First valid SessionStart binds the
native session; turn events must match the latest UserPromptSubmit UUID. Native
input immediately revokes semantic confidence and establishes a timestamp/output
barrier. Late older-turn receipts cannot restore it. SessionStart may arrive
when startup trust is finally accepted, so its identity handshake is independent
of the input barrier. The recognized native header pins modal/completion
classification to 0.167.0. Signals use at most 30 seconds of freshness; Working
uses five. Missing trust, unsupported versions/layouts, invalid or misattributed
receipts, ambiguous tool intervals, excessive evidence backlog, changed screens,
non-running execution and expired evidence produce unknown.

A five-second model request can still be working after the evidence expires;
unknown is deliberate. Completion also stays unknown when a new marker cannot
be distinguished from the preceding turn's identical marker, or the idle
composer/footer is unavailable. Narrow/wrapped layouts and other terminal
configurations are not additional status support claims. These rules prefer
missed labels over invented outcomes. They are observability safeguards, not a
security boundary against another process running as the same macOS user.

## Native execution and negative checks

The final complete run included execution-gate integration commit 7455825 and
shared Claude monitor integration daf1005. The real dashboard adopted
`/private/tmp/rat-codex-proof-checkout`, launched `x` once, and detached. A Python
PTY driver then controlled that same supervisor through its public request
protocol, collecting its native parsed screen and status on every scenario.
There was one native coding child, using `--no-daemon`; no alternate integration
mode supplied status. Final state directory was `/private/tmp/r12z`.

Managed identity was task `93a1c122-2643-43d1-b113-12f531c8cf90`, session
`edb9095d-770d-480e-a709-dac73ee98232`, request
`c20f82f2-3c2d-43e1-a2ce-5a373da109ab`. A separate minimal SessionStart observer
recorded native conversation `01a1b118-f581-7e52-b5e8-b5a84b111a4e`.

1. Review/trust the invocation-local hooks in Codex's native prompt.
   [Before trust](screens/12-codex-hooks-review.json), semantic status was unknown.
   Production hooks call only the bounded passive helper and return no stdout,
   decisions, context or approval changes. Existing user hooks/configuration,
   credentials and account policy were retained; no bypass flags were used.
2. Submit `Reply exactly first-gated-stop-proof. Do not use tools or write files.`
   The proof alone added one reviewed invocation-local Stop handler that emitted
   `{"decision":"block","reason":"Reply exactly continuation-gated-proof. Do not use tools."}`
   once, then `{}` thereafter. Native `Blocked by hook` and the follow-up reply
   appeared. The same turn emitted PreModelRequest/PostModelRequest/Stop twice.
   Polling observed no Completed before the second Stop; only its new done
   marker and idle composer established Completed. Production monitoring never
   includes this continuation handler.
3. Ask for literal `Worked for 999s • 00:00`, `Conversation interrupted`, and
   `Would you like to run the following command?`, followed by
   `quoted-proof-done`, without tools. No Input, Approval or Interrupted label
   appeared while those phrases were ordinary output. Only the actual final
   native completion was classified. [Quoted phrases](screens/12-codex-quoted-negative.json).
4. Enter `/plan`, request request_user_input with alpha/beta choices and wait
   for Input. Leave the question unanswered for over 30 seconds: the same
   pending native question became [unknown/stale](screens/12-codex-question-stale.json).
   Enter answered alpha; matched PostToolUse and final completion followed.
   [Answered question](screens/12-codex-question-answered.json).
5. Shift-Tab returned to normal mode. Ask for exactly
   `printf codex-status-gated > status-codex-gated-marker` with require_escalated
   and a justification. The actual approval modal produced Approval. Enter
   approved once, without granting a persistent prefix. The command ran and
   [completed](screens/12-codex-approval-completed.json). No other command was
   requested in this scenario.
6. Ask to count to 100000 without tools. After a genuine PreModelRequest,
   forward Ctrl-C. PostModelRequest and Interrupt confirmed cancellation.
   Repeat with a new turn and deliver a sanitized Stop receipt carrying the
   previous interrupted turn ID through the same public hook helper. It revoked
   confidence to [unknown](screens/12-codex-delayed-negative.json), never Completed.
   Interrupt the second active turn normally.
7. Type `/exit`, allow native command parsing, then Enter. Native SessionEnd
   and Process Exited retained task attribution with semantic unknown.
   [Native exit](screens/12-codex-exited.json). Explicit Shutdown then reported
   stopped=true. Three preliminary isolated proof supervisors were also
   explicitly stopped. An earlier harness sent `/exit` and Enter in one write;
   it left `/exit` in the composer, correctly remained Running/unknown and was
   explicitly shut down. The final paced run above exited normally.

Installed help/version were inspected before implementation. The fetched
[official hooks reference](https://learn.chatgpt.com/docs/hooks) documents native
hook configuration, trust, diagnostic model-request hooks, Stop continuation
and Interrupt. Documentation was corroborating research; only the native
receipts/screens above establish this version's result. Hook merging preserves
other active layers. Fresh invocation tokens change the hook definition, so
native trust review may be required again on a later launch. Hooks disabled by
policy or not trusted leave status unknown without blocking ordinary launch.

## Implementation and verification

`codex_status` uses the shared State/Status event/render vocabulary. Its bounded
private datagram channel receives at most 2048-byte sanitized identity receipts,
32 per supervisor pass, retains 16 recent receipts and at most 16 pending tools.
The helper accepts at most 65536 input bytes, discards prompt/tool payloads and
credentials, and emits no policy output. Hook failures do not prevent native
interaction. Monitoring and launch configuration stay outside the UI loop;
the durable execution gate preserves every native argument. Detached monitors
refresh the parsed screen on a changed output boundary rather than evaluating
an old controller snapshot. Status remains ephemeral and is not restored as a
current semantic claim from history.

The approved real dashboard/supervisor/scripted-child test was written first
and failed because the Codex evidence channel/visible availability reason did
not exist. It passed after implementation. It verifies unknown despite a live
process, ordinary status prose, malformed evidence and an intermediate failed
fixture command. The existing shared fixture exercises all visible state labels,
state-event separation, expiry, wrong-session evidence, post-tool invalidation
and continued work after a failed child command. App/render checks retain
readable semantic labels separately from process lifecycle and connection;
completion does not archive or stop a task.

An integration run exposed the existing partial-write JSON EOF in the bounded
history test. Ticket 13's cc97e30 transport fix was merged rather than weakening
that test. Locked build, formatting and warnings-denied Clippy passed after that merge.
The next full suite passed the former history failure and the Codex regression,
then stalled in the older bounded-PTY cleanup harness. That run and only its
known fixture were explicitly stopped. Ticket 13 cleanup checkpoint a3841db and
fresh integrated full checks remain pending; ticket 12 stays unresolved until
that verification completes. See the ticket's verification comment.
