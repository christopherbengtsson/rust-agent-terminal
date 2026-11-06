# Ticket 10: Claude status evidence — partial proof, support gate open

2026-11-06, macOS 26.2 (25C56), arm64; stable Rust 1.99.1. This changes
semantic observations, not process ownership, connection ownership, archival or
native policy. **Ticket 10 remains unresolved.** Native input and Bash approval
have evidence; working, completed-turn and interruption classifiers remain open.
This is not a claim that those remaining signals are impossible.

## Native execution and receipts

The real dashboard and separate supervisor used macOS PTYs at 160×40, with a
104×32 child pane, portable-pty 0.9.0, vt100 0.18.1 and Ratatui 0.30.2. No
Terminal.app/iTerm compatibility or graphical screenshots are claimed. The
isolated adopted checkout was `/private/tmp/rat-claude-proof-checkout`; private
supervisor directories were `/private/tmp/r10s`, `/private/tmp/r10s2` and
`/private/tmp/r10s3`. Earlier proof instances were explicitly shut down before
the final instance. Native credentials remained with Claude.

The launch PATH resolved through a proof-only symlink to the canonical existing
`~/.local/share/claude/versions/2.1.318`, SHA-256
`677a2ea7bf576cedd20e191d973914ed8a41796f53a63b1923381ddc4d5b5e7d`
(recorded in ticket 05). The UI reported 2.1.318. The installed default symlink
had updated to 2.1.319; it was not changed by this proof. Native model: Haiku 5.5,
medium effort. No model/version substitution was inferred from old research.

For each invocation the supervisor creates private `status.sock` and
`status-settings.json` in its own state directory, generates a fresh native
session UUID and generation token, then adds `--session-id` and `--settings`.
The passive command helper returns no decisions, context, stdout or stderr and
always exits successfully. It does not replace settings sources, disable hooks,
change permissions or authentication, or edit user/repository configuration.
The existing nonblocking SessionStart failure referencing
`${CLAUDE_PLUGIN_ROOT}/hooks/run-hook.cmd` remained visible in the first monitored
session, alongside a successful independent monitor SessionStart receipt.

The [official hook reference](https://code.claude.com/docs/en/hooks) describes
pre-decision events, concurrent/custom hooks, and Stop continuation. Consequently
receipts alone are insufficient evidence of a pending human dialog or final
completion. The implemented attention mapping also requires the live native
modal. Generic Notification does not close an open tool interval.

| Scenario | Actual receipt and native screen | Product observation |
| --- | --- | --- |
| Startup | SessionStart, matching generated session; native 2.1.318 banner | Unknown; startup/liveness does not imply working |
| AskUserQuestion | PreToolUse with unique tool-use ID; PermissionRequest naming AskUserQuestion without a tool-use ID; delayed Notification; native question modal with options and selection/cancel footer | **Input needed**; [receipt and parsed screen](screens/10-claude-input.txt) |
| Answer question | Enter selected Alpha; matching PostToolUse ID, PostToolBatch, Stop; native answer and done marker | Tool interval closes; semantic status becomes unknown |
| Bash approval | PreToolUse Bash ID, PermissionRequest Bash without an ID; delayed Notification; exact native Bash permission modal | **Approval needed**; [receipt and parsed screen](screens/10-claude-approval.txt) |
| One-time approval | Enter selected Yes once; matching PostToolUse, PostToolBatch, Stop; native answer/done marker | Approval clears; unknown rather than treating Stop as final completion |
| Command failure | Requested standalone `sleep 30`; Claude's own native tool harness blocked that command and the agent explained the failure, without a product approval rejection; process remained Running | Unknown; [receipt and screen](screens/10-claude-command-failure.txt). No workaround was attempted |
| Native Ctrl-C | Repeated the ticket 05 counting prompt (1–1000, no tools requested); this session nevertheless ran one Bash tool, emitted PreToolUse/PostToolUse/PostToolBatch, then displayed `Interrupted · What should Claude do instead?` after Ctrl-C, without Stop | Unknown; [receipt and parsed screen](screens/10-claude-interrupted.txt). Native acknowledgement exists, but no classifier/quoted-marker negative proof is claimed |
| Long response attempt | Requested 1–100000 without tools; native agent chose to give 20 lines and a normal answer instead | Stop + native done marker observed; this did not establish an active long-response interval |

The marker command was exactly `printf status-final > status-final-marker` in
the isolated checkout. Manual mode was selected only for the native proof, and
only the one-time Yes was chosen; no always-allow choice was selected. Ordinary
attach, question input, approval input and detach used the real dashboard. Text
captures contain sanitized hook identities and parsed child screen content;
fixture rendering checks establish the actual label presentation separately.

## Signal validity and conservative invalidation

A receipt must fit 2048 bytes, match the fresh generation token/native session,
come from a main-agent context and contain a recognized event. The helper reads
at most 65537 input bytes and discards prompt/tool payloads, results and native
transcript paths. It retains only recognized event names, AskUserQuestion/Bash
names and bounded tool/prompt identities. There are at most 16 pending tools and
16 recent receipt summaries. The supervisor consumes at most 32 datagrams per
pass. Backlog uncertainty, malformed/wrong-session receipts or channel errors
revoke semantic confidence; overload never implies completion or failure.

A PermissionRequest has no tool-use ID. It is attached only to exactly one
pending main-agent tool of the same sanitized name and prompt identity. Ambiguous
parallel calls remain unknown. PostToolUse/PostToolUseFailure closes the exact
ID; UserPromptSubmit, SessionEnd and ConfigChange clear pending intervals. A
question may pass through PermissionRequest; that event does not make it a Bash
approval. Notifications do not reset the age of the pending tool.

Attention requires the observed 2.1.318 native banner and a canonical executable
whose version filename is `2.1.318`; other install layouts/versions remain unknown.
The executable is resolved once from inherited PATH and launched through that
canonical path, avoiding an updater symlink changing between attribution and
spawn. These are compatibility checks, not a security attestation against a
malicious same-user executable. Current main-agent tool evidence must be no more
than 30 seconds old and agree with the current modal structure, option rows,
borders and exact native footer. Input also requires the question heading;
approval requires the Bash heading, proceed question and Yes option. A terminal
phrase in ordinary output is insufficient. No generic prose-question detection
is claimed. Narrow/changed modal layouts fail to unknown.

Forwarded native input revokes confidence until the parsed output boundary moves;
a matching modal may then reestablish it. An unresponsive unchanged screen does
not acknowledge input. Missing evidence starts unknown; fixture evidence expires
after five seconds and native tool intervals after 30 seconds. Expiry only revokes
confidence. It never means completed, idle or failed. Process stop/exit similarly
clears semantic state. Persistence saves/restores unknown semantic state; live
hook tokens/pending intervals do not survive supervisor restart. A monitor setup
failure leaves native launch usable with unknown status.

## Open proof gates and proposed resolution

- **Working:** UserPromptSubmit precedes processing and another hook may block it.
  A PreToolUse request can also be blocked, as the native harness failure shows.
  No current-generation spinner/tool-running structural classifier has passed
  negative cases here. Add a bounded receipt-correlated native active-UI matcher;
  test blocked submission/tool hooks and output quoting that UI before assigning
  Working. A live process, output volume or silence cannot substitute.
- **Completed:** Stop and the native done marker were both observed. Completion
  is still unverified under a Stop hook that continues the turn, delayed receipts,
  a new submitted prompt, and quoted old done markers. A future classifier should
  require a current-turn receipt, the final native prompt/done structure after
  hook resolution, and those continuation/quotation negatives. Stop alone remains
  an observed fact, not a completed-turn status.
- **Interrupted:** Ticket 05 established the native interrupted screen during a
  long response. This monitor has no demonstrated general interruption hook.
  Forwarded Ctrl-C alone is only a request; question Ctrl-C did not supply a
  reliable new acknowledgement in the monitored scenario. Correlate a new native
  interrupted UI acknowledgement with the input/output generation, then test model
  generation, running tools and dialog cancellation separately. Do not infer
  interruption from absent Stop, liveness, or a quiet PTY.
- **Coverage:** plain-text follow-up questions, approval denial/autoapproval,
  managed-hook suppression, blocked Stop/UserPromptSubmit, subagent concurrency,
  API/auth failures, additional terminal apps and arbitrary native UI versions
  remain unverified. Missing or ambiguous evidence stays unknown. Required gates
  must pass before claiming native status support or resolving this ticket.

| Version | Native interaction | Status support |
| --- | --- | --- |
| Claude 2.1.318, canonical hash above | Ticket 05 + monitored question/approval/one-time answer here | Partial input/Bash approval; required semantic support gate open |
| Claude 2.1.319 | Installed; no full runtime status proof | Unknown; no 318 modal confidence inherited |
| Claude 2.1.74 | Stale exploratory research only | Unsupported by this proof |

## Fixture/state verification and visual direction

TDD used the already approved App/event and real dashboard/supervisor/scripted
PTY seams in real temporary Git checkouts. The first state test failed to compile
before semantic observations existed. PTY scenarios then failed before the
fixture evidence commands, wrong-session rejection, failed-command handling and
correlated native-style question/approval behavior existed, and passed after the
corresponding implementations. A normal quoted native status phrase cannot create
a semantic observation. Tests cover all six labels, missing/stale and unattributed
evidence, closing a question interval, and a failing intermediate command while
fixture working evidence remains current. These are adapter handling tests,
not native support evidence.

The normal dashboard keeps reviews above the project/worktree sidebar and the
native terminal on the right. Labels are readable text independently of color;
semantic status is separate from Process and input focus. Unknown/revocation
reasons appear in the Status border and native evidence in its body. At small
sizes the sidebar collapses and status remains in compact help; the first release
still differs from the reference image by using one native session rather than
an application message composer. No new graphical-terminal visual proof is
claimed from these parsed screen/PTY tests.

Final verification after merging integration `f511471`: `cargo build --locked`,
`cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`
and `cargo test --locked --all-targets` passed. Tests: 19 unit tests passed,
one ignored subprocess probe exercised by its passing restoration test; 20
ownership/history scenarios passed in 35.44s; scripted interaction/status proof
passed in 15.72s; screen proof passed in 8.71s. These are verification runtimes,
not performance promises. Status receipts are ephemeral; durable native output
continues through ticket 08's independent history capture.

## Reproduce the partial native attention proof

1. Build with `cargo build --locked`. Use a fresh empty Git checkout and a
   private isolated state directory. Put a symlink named `claude` to the recorded
   canonical 2.1.318 executable first in the proof process's PATH; do not change
   the user's installed symlink. Run `target/debug/agent-terminal <checkout>
   --state-dir <private-proof-state>` in a 160×40 PTY.
2. Adopt with `a`, launch with `c`, accept native folder trust only for that empty
   checkout if needed. Ask: `Use AskUserQuestion to ask whether the proof marker
   should be alpha or beta. Do not write files or run commands.` Observe Input
   needed during the live modal within the 30-second validity window.
3. Answer Alpha. Select manual mode through native Shift-Tab. Request exactly
   `printf status-final > status-final-marker` in the isolated checkout. Observe
   Approval needed in the Bash modal; Enter approves once. Check the resulting
   marker and that neither attention state persists after its tool returns.
4. Ctrl-] returns to the dashboard; `d` detaches. Reopen against the same state
   directory to inspect the same session and current evidence. Older evidence
   must be unknown rather than indefinitely claiming attention.
5. To reproduce the native interruption screen, request `Count from 1 to 1000,
   one number per line. Do not use tools.` and send Ctrl-C while active. Record
   what the native tool actually does; this run invoked Bash despite the prompt.
   The screen acknowledgement is captured, while product semantic status stays
   unknown pending the negative-case classifier proof.
6. Explicitly stop/quit the isolated proof supervisor. All three state directories
   used above were shut down with confirmed `stopped=true`; no proof child was
   intentionally left running. Do not stop or connect to the user's usual state
   directory as part of a proof.
