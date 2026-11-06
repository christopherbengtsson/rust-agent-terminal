# Ticket 10: bounded Claude native status evidence

2026-11-06, macOS 26.2 (25C56), arm64, stable Rust 1.99.1. The five required
semantic observations were demonstrated with Claude Code **2.1.318** in native
PTY mode. Unknown remains the result outside the demonstrated bounds. Process
ownership, connection state, history, native resume and archival stay separate.

The canonical binary was
`~/.local/share/claude/versions/2.1.318`, SHA-256
`677a2ea7bf576cedd20e191d973914ed8a41796f53a63b1923381ddc4d5b5e7d`.
The installed default 2.1.319 symlink was preserved and is not covered. Native UI
reported Haiku 5.5, medium effort. The outer real dashboard ran in a 160×40 macOS
PTY with a 104×32 child pane, portable-pty 0.9.0, vt100 0.18.1 (bounded OSC parser),
and Ratatui 0.30.2. These are parsed PTY captures, not graphical-terminal images.

## Native observations

The real dashboard explicitly adopted/launched the empty isolated checkout.
The public supervisor connection then drove native input and recorded parsed
screens, output boundaries, semantic observations and process lifecycle. Initial
runs used `/private/tmp/rat10c-checkout` and `/private/tmp/r10c`; final merged-code
checks used `/private/tmp/rat10final-checkout` and `/private/tmp/r10final` after
merging integration `e2cfef3` (ticket branch merge `06b1149`). Native settings and
credentials remained native. Proof-only PATH symlinks selected the canonical
binary without changing the user's installed default.

[Raw observation excerpts](10-native-observations.jsonl) retain native event
summaries and parsed screens at state/event transitions, plus the final sample
of each scenario. They omit repeated unchanged polls. Companion text captures
make the positive states and important negatives easier to inspect.

| Scenario | Actual native evidence | Visible outcome |
| --- | --- | --- |
| Startup/pre-output thinking | SessionStart/UserPromptSubmit, live process, animated thinking footer without new response | Unknown; liveness, submission and a spinner alone are insufficient |
| Streamed long explanation | Current UserPromptSubmit; new `⏺ Rust Ownership…` response; native `esc to interrupt` footer; advancing parsed output | **Working**; [capture](screens/10-claude-working-final.txt) |
| Model interruption | Controller waited for Working, sent real Ctrl-C; a new `⎿ Interrupted · What should Claude do instead?` appeared with idle composer; no Stop receipt | **Interrupted**; [capture](screens/10-claude-interrupted-final.txt) |
| Short response completes | Current-turn Stop; later parsed boundary; new native `✻ … for … · done …` marker and idle bordered composer | **Turn completed**; [capture](screens/10-claude-completed-final.txt) |
| Native question | Unique main-agent AskUserQuestion PreToolUse, matching PermissionRequest without tool ID, actual question/options/footer | **Input needed**; [capture](screens/10-claude-input-final.txt) |
| Native Bash permission | Unique Bash PreToolUse and correlated PermissionRequest; actual Bash permission modal | **Approval needed**; [capture](screens/10-claude-approval-final.txt) |
| One-time approval | Enter selected Yes once; exact PostToolUse closes interval; marker file contains `classifier-ten` | Attention clears; no always-allow or policy bypass was selected |
| One-shot Stop continuation | First Stop while a proof-only native Stop hook asked for continuation; native subsequently answered `continued-stop-ten`, then emitted a second Stop/new final done/idle structure | First Stop **Unknown**; only second/final Stop becomes **Completed**; [capture](screens/10-claude-stop-continuation.txt) |
| Blocked submission | Proof-only UserPromptSubmit hook exited 2; native displayed its blocked-operation explanation/restored composer | All captured samples **Unknown**, including monitor UserPromptSubmit receipt |
| Blocked tool | Proof-only PreToolUse hook exited 2; native displayed Not run / permission denied | Pending tool interval **Unknown**; earlier genuine streamed explanation can be Working and later genuine final Stop can complete |
| Failed intermediate command | Native Bash ran `false`, emitted PostToolUseFailure; agent explained exit status 1 | Process remained **Running**; no failed-session state was invented; [capture](screens/10-claude-failed-command-final.txt) |
| Question Ctrl-C/Escape | Ctrl-C alone did not close this native question. Documented Escape cancellation displayed the same native interrupted phrase and a done-looking marker, without final Stop | **Unknown**, never Interrupted merely from dialog cancellation; [capture](screens/10-claude-modal-cancel-final.txt) |
| Quoted old markers | Native answer quoted old done/interrupted examples; earlier native acknowledgments were also still visible in history | Neither quote nor old history created an Interrupted or pre-Stop Completed observation; actual final current-turn Stop may legitimately complete |

The isolated one-shot blocker script was attached only through the temporary
checkout's `.claude/settings.local.json`. Its Stop handler consumed an armed
one-shot file and returned `{"decision":"block","reason":"…continue…"}`.
Submission/tool handlers consumed their respective one-shot files and exited 2.
No user/global/repository-under-development hook or permission policy was edited.
The standalone sleep command rejected in the earlier partial proof was not retried
through a workaround. Long native generation used a harmless explanation prompt.

## Attribution and conservative bounds

For each launch the supervisor adds private invocation-local settings, a fresh
generation token and native session UUID. Resume preserves the original native
UUID while creating a new monitor token. The passive command hook emits no
policy decisions, stdout, stderr or context and always exits successfully. It
reads at most 65537 bytes, accepts at most 65536, discards prompts/tool payloads,
results, credentials and transcript paths, and projects bounded event/tool IDs.
The [official hook reference](https://code.claude.com/docs/en/hooks) explains why
pre-decision events and Stop continuation need independent UI corroboration.

Receipts fit 2048 bytes, match token/session/main-agent attribution, have valid
local timestamps within 30 seconds, cannot precede the current native-input or
submitted-turn boundary, and agree with prompt identity when the native CLI
provides one. Claude often omits prompt IDs; a bounded invocation-local submitted
turn, input/time barriers and current UI provide the demonstrated correlation.
This is per-version/scenario reliability, not perfect same-user security
attestation against arbitrary full-screen or hook-protocol forgery.

The monitor consumes at most 32 datagrams per pass, retains at most 16 recent
summaries and 16 pending tools, and uses the already bounded parsed viewport.
Invalid, wrong-session, stale, ambiguous or overloaded receipts revoke confidence.
An unrecognized tool still prevents a pending interval from being mistaken for
active model output. A generic Notification cannot close a tool interval or
refresh its original 30-second age.

- Attention requires exactly one fresh matching pending tool, the demonstrated
  modal heading/options/borders/footer, and a later parsed boundary after input.
- Working requires current submitted-turn evidence no older than 30 seconds,
  output advancing within five seconds, a response header different from the
  submission/tool baseline, the actual active footer and no pending tool.
  Pre-output thinking, blocked hooks, old response headers and silence stay
  unknown. Long/wrapped responses whose identifying header has scrolled away
  can also become unknown.
- Completion requires current-turn Stop and an output boundary after its receipt,
  a new done marker compared with the submission baseline, and the final idle
  bordered composer. The region between marker and composer contains only blank,
  border or demonstrated native update rows. Native continuation without final
  corroboration cannot complete. New input/prompt/tool activity invalidates Stop
  confidence. Completion expires within 30 seconds of supporting evidence.
- Interruption requires an active native footer when Ctrl-C is requested, a later
  output boundary within five seconds, an idle composer and a newly observed
  acknowledgment at the native tail. Its count must exceed the pre-request count;
  an already printed/quoted acknowledgment cannot satisfy the request. Dialog
  cancellation and Ctrl-C alone do not qualify. Acknowledgment expires after
  30 seconds and any later input revokes it. If scrolling makes a new marker
  indistinguishable from an old one, the result is conservatively unknown.

A native banner plus canonical filename `2.1.318` gates these specific shapes.
Changed/narrow layouts, pre-output thinking, arbitrary prose questions,
unrecognized permission tools, missing hooks, ambiguous concurrent calls, native
composer suggestions/other update layouts, additional versions, graphical terminal
apps and untested failure paths remain unknown. These bounds intentionally prefer
unknown to inventing a result. Monitoring failure does not prevent ordinary
native launch; process stop/exit clears semantics; persistence restores unknown
rather than resurrecting old current-state claims. Native output/history capture
remains independent of this ephemeral channel.

## Regression tests and verification

TDD used the already approved App/event/render and real dashboard/supervisor/
scripted-PTY seams in temporary real Git repositories. Completion, active-response
Working and acknowledged interruption each failed at the visible dashboard seam
before their classifiers existed, then passed. Existing shared tests continue to
exercise all labels, process/transport separation, wrong-session evidence, expiry,
question/permission closure and an intermediate failed test while work continues.
Additional scenarios cover Stop continuation, a new prompt with an old done
marker, stale delayed Stop, native-looking quoted done/idle output without current
receipts, an old acknowledgment across Ctrl-C, active-tool interruption and modal
cancellation. Fixture evidence establishes handling, not native support.

Readable text labels remain separate from Process and focus, in the sidebar and
status/session borders. App/render tests cover normal and small layouts; the
native screen stays prominent on the right and reviews remain honestly
unavailable. This follows the reference image's readable-label direction with the
already documented one-session/native-screen differences. No new graphical
terminal screenshot or broader release-readiness claim is made.

The full locked suite before the final parser merge passed: 22 unit tests, one
ignored subprocess probe exercised by the passing restoration test, 32 ownership
scenarios (193.43s), scripted interaction/status (26.03s), screen integration
(8.66s). After merging final parser integration `e2cfef3`, locked build, formatting,
warnings-denied all-target Clippy, 22 unit tests, the expanded scripted proof
(26.64s) and screen proof (8.68s) passed. The final integrated resource/33-ownership
union is covered by ticket 18; the already completed 140-second resource workload
was not repeated merely for documentation edits. A subsequent submitted-turn boundary
regression failed before the explicit timestamp/prompt guard, then passed (27.07s).
All five native positives were then repeated with that final guard; the one-time
approval produced `final-guard-ten` in `proof-guard-marker`, and explicit shutdown
confirmed `stopped=true`. All proof supervisors were explicitly stopped. These runtimes are test durations,
not performance promises.

## Reproduce

1. Build `cargo build --locked`. Create an empty temporary Git checkout and a
   private state directory; prepend a proof-only `claude` symlink to the recorded
   canonical binary. Run the real dashboard in a 160×40 PTY, adopt and launch with
   `a`, `c`, and explicitly trust only that empty checkout if native trust asks.
2. Request a detailed Rust ownership explanation without tools. Observe Unknown
   during pre-output thinking, then Working during a new streamed response. Send
   Ctrl-C only after active output and check a new native acknowledgment before
   accepting Interrupted. Send a short exact-response prompt and observe final
   Stop/new done/idle composer before accepting Completed.
3. Ask through AskUserQuestion and observe Input. Escape cancels that modal;
   native interrupted-looking text by itself must remain unknown. Select native
   manual mode with Shift-Tab, request the harmless temporary marker command,
   observe Approval and choose Yes once; verify attention clears.
4. In a separately isolated checkout/settings layer, use one-shot native blockers
   to reproduce blocked submission/tool and Stop continuation. Record the actual
   receipts/screens: first Stop is insufficient. Quote old markers in a later
   native response and verify no outcome appears solely from those strings.
5. Ctrl-] returns to dashboard navigation; detach/reopen keeps the same managed
   session. Explicitly stop/shut down every isolated supervisor after the proof.
   Never connect to or stop the user's usual supervisor directory.
