# Milestone 2 ticket 12: selected native conversation recovery

2026-11-07, macOS 26.2 (25C56), arm64. Scope: explicit older/latest
conversation selection through the original tool and native UUID, with a fresh
managed segment in the retained workspace. Archive/removal and the final physical
reboot/resource gate remain separate work.

## Implementation and persistence boundary

`ResumeSelected` freezes the launch request, workspace and original managed
segment UUID. These differ from the transient runtime control target, original
native conversation UUID and new managed segment UUID. A sole-writer lookup
validates workspace/task aliases, membership, row/decoded UUID consistency,
supported original tool and native UUID. It returns a small immutable descriptor.
Historical Observations are never installed in the live owner. The chosen tool
and native ID are set after allocating the new segment and before durable
Prepare. Prior native monitors are retired at that generation boundary.

Schema 9 adds independent request-to-source bindings and durable cancellation.
A changed source cannot reuse a bound request after restart. Transcript deletion
keeps recovery metadata. Lookup reserves the canonical checkout, reports its
frozen request to reconnect queries, and is disarmed by Stop. Cancellation holds
its reservation until the writer acknowledges it. A late lookup cannot launch.
After lookup, the supervisor re-enters its ordinary launch admission with current
setup/occupancy/capture reservations before Prepare/create/authorize/result.
The existing global same-request outcome recovery remains in use.

`B` lists saved conversations with their workspace, original tool, segment and
native IDs. Explicit `v` resumes the selected row. Dashboard `v` freezes the
latest managed segment and uses the same resolver. Enter/history/search/paging
and raw/readable toggles remain read-only. An occupied workspace selects its
reserved owner and offers attachment without automatically taking native focus.
Another conversation requires explicit Stop and settled prior capture.

Missing source/membership, unsupported tool, missing native UUID and missing
checkout have actionable errors. Native record/version rejection stays in the
captured native UI of the attempted new segment. `c`/`x` remain explicit new
conversation choices. No launch is described as successful native recovery merely
because its process is running. No Git rewind or cross-tool state transfer occurs.

## Actual native marker proof

Real supervisor/Controller transport and portable-pty native children at 100×24,
TERM `xterm-256color`. The Rust dashboard PTY and keyboard/render paths are
covered separately by the fixtures and App/TestBackend checks below. These are
parsed native PTY captures; no graphical terminal application or physical reboot
is claimed.

Fresh proof-only checkout `/private/tmp/r12n2/checkout`, product state
`/private/tmp/r12n2/state`. Existing native authentication stayed in place.
Native folder trust was explicitly accepted for this created repository. Codex's
10 invocation-local passive hooks were inspected in its native Hooks screen and
trusted there for each changed invocation. No bypass flags or credential copies
were used, and no native records were deleted/moved to simulate absence.

| Tool | Canonical binary | Version and SHA-256 |
| --- | --- | --- |
| Claude Code | `~/.local/share/claude/versions/2.1.319` | `2.1.319`, `1f53587e92dee8e089fa349188a4cda64fa34ace6e7b0ca2ee278ddaa3cc2ebd` |
| Codex CLI | `~/.codex/packages/standalone/releases/0.167.0-aarch64-apple-darwin/bin/codex` | `0.167.0`, `030316ecfbb41f4e7946e584b0063ef333b7a36cd312ba349f2f6869e39b62cc` |

Installed `claude --help` and `codex resume --help` established the supported
explicit-ID invocation shapes. Claude displayed Haiku 5.5 medium; Codex displayed
GPT-6.1-Sol medium. The default Claude binary was 2.1.319 for this proof; the
older ticket 16 proof's pinned 2.1.318 is not reused as evidence.

Original marker conversations were created with product source
`7160a80355d0291b8e1aaef4c70e2bc889828843`, binary
`/private/tmp/rat-ticket12/target/debug/agent-terminal`, SHA-256
`e64cd254b27d968e134f2f7bdb8f544d1f1b1beade8d3911f089d242e7c3ac49`.
After all four exited, that isolated supervisor shut down. Recovery restarted the
same private state using the final selected-recovery implementation at
`39dfd0d73fddb36d43a0c7d7638a915a09df6010`, including final ticket 10 product
source `343c561`; binary SHA-256
`998f9265aa77356022a7292cdea8d24ecd5fcf167c2104573d84985f095ca205`.
Subsequent changes add malformed-request bounds, rendering/fixture checks and
documentation without changing native argv construction or marker behavior.

All conversations retain workspace UUID
`97775705-12b3-4c9b-a054-2bc8800d869b` and its opaque task identity
`0713e1b7-14bc-4141-a2fc-86d9b46c341d`. Initial creation order was Claude A,
Codex B, Claude C, Codex D. Each was given a distinct conversation-only marker
with an explicit instruction to answer STORED and avoid tools/files.
Explicit resume order was **B, A, D, C**, selecting original managed segment IDs
rather than a latest-session picker or screen offset.

| Selection | Original native conversation UUID | Original managed segment → resumed managed segment | Marker returned by new response |
| --- | --- | --- | --- |
| Older Codex B | `01a1b352-b21a-7e42-b697-c045abcaa225` | `3d81161d-9d1f-41e1-b9ea-4602a0dafe21` → `db81e739-30d8-4000-a370-55a86eba01e5` | `x12-violet-6f2c` |
| Older Claude A | `e203da71-3dd8-4c9a-9361-7577fb0423ba` | `3b66e827-f6d1-4adc-94fc-b631326b0496` → `698b035b-bee3-4ffd-a156-584c57ae1887` | `c12-amber-8d7e` |
| Newer Codex D | `01a1b354-7c93-7fb3-802a-3bd3b3ba281f` | `803bcc52-894b-49b0-b987-375a56ca9a61` → `f6e6670e-f268-4181-9046-4f9b05bd72a1` | `x12-cobalt-14d0` |
| Newer Claude C | `cab48ef1-3e6d-4cbf-830a-ca10d201ba61` | `004a769d-714d-40da-a264-b78e25d3c327` → `9d5a3a39-7174-4369-8b5a-56545b57db95` | `c12-copper-92a1` |

For each recovery, process argv showed the original native ID:
Claude `--resume <original-id> --settings <invocation-monitor-file>` without a
conflicting `--session-id`; Codex `resume <original-id> --no-daemon` plus the
existing invocation-local monitor `-c` settings. Neither included a follow-up
prompt. Captures before input show native replay/trust and no new user turn.
Only then was this explicit prompt submitted:

> What exact conversation-only token did I ask you to remember earlier? Reply with that token only. Do not use tools or read files.

That prompt contains no marker. All four new responses returned the distinct
original marker. This proves original-conversation continuation beyond terminal
replay, including two changes of tool in the same workspace. Claude's semantic
status remained Unknown; it was not promoted to Completed based on process
liveness or this proof. Codex's normal current-turn evidence remained separately
version-scoped.

[Native IDs, source/binary manifests, argv, cwd, status and cleanup summary](m2-12-selected-native-summary.json)
contains the exact records. Monitor nonces are redacted from recorded argv;
conversation IDs and selection flags remain exact. Native headers identified the
proof cwd for all runs; `lsof -d cwd` additionally checked resumed A/C/D.
Before-prompt and recall captures:
[B before](screens/m2-12-codex-b-before-prompt.txt), [B response](screens/m2-12-codex-b-recalled.txt),
[A before](screens/m2-12-claude-a-before-prompt.txt), [A response](screens/m2-12-claude-a-recalled.txt),
[D before](screens/m2-12-codex-d-before-prompt.txt), [D response](screens/m2-12-codex-d-recalled.txt),
[C before](screens/m2-12-claude-c-before-prompt.txt), [C response](screens/m2-12-claude-c-recalled.txt).

Before recovery the checkout contained staged and unstaged edits to `tracked`
and an untracked file. After all four recoveries, HEAD
`f52a44ce969aa9e24d4f4a7e07cc7cc6fff74048`, staged diff, working diff, untracked
contents and `git status` (`MM tracked`, `?? untracked`) were unchanged.
Each native CLI exited through `/exit`; final product shutdown acknowledged
`stopped=true`, `metadata_pending=false`, `execution=null`. A subsequent `ps`
query found none of the eight observed native leader PIDs or two isolated
supervisor PIDs. This is not a host-wide resource/escaped-descendant qualification.

An initial exploratory launcher omitted daemon detachment and lost its isolated
supervisor before a marker turn. It was excluded. All reported marker cases use
`r12n2`, with detached supervisor ownership and the cleanup receipts above.

## Fixture and state verification

The approved real Git/Worker/Controller/dashboard/PTY seam covers:

- Claude A, Codex B, Claude C followed by explicit B/A/C/B selection. The wrapper
  argv records the original tool/native ID, no follow-up or new-session-ID
  conflict, fresh managed segment and unchanged workspace. Repeated B selection
  creates multiple segments sharing its original native conversation.
- A stalled durable Prepare already contains the selected B tool/native ID.
  Same-request delivery with a different source cannot replace it. Committed
  same-request replays return the recorded segment and do not spawn again.
- Lost acknowledgement during descriptor lookup exposes the frozen request on
  reconnect; QueryLaunch stays pending. Stop disarms lookup, late completion
  cannot launch, and replay of that durably cancelled request is refused.
- Occupied worktree refusal offers attachment. Existing canonical-alias and
  uncertain prior-boot execution tests preserve their barriers. The final setup
  admission is shared before and after the lookup.
- Missing source membership, original tool and native UUID are explicit. Temporarily
  moving this test-only checkout blocks selected resume while its raw history
  stays readable; restoring it permits the next explicit choice. The original raw transcript remains
  identical after later selected resumes. HEAD, staged/unstaged diff and
  untracked contents remain unchanged.
- A fake native CLI rejects the exact original-ID resume with a bounded
  conversation-unavailable/unsupported-version diagnostic and exit 7. The new
  attempted segment retains it without an automatic replacement. This is a
  fixture negative case, not proof that a real native record was removed or a
  real version rejected recovery.
- The Worker test resolves recovery metadata after transcript deletion, restarts
  storage, rejects changed-source/cancelled requests and still resolves the same
  original tool/native ID with a fresh explicit request.
- App tests freeze browser workspace/segment identities and keep occupied native
  focus explicit. Rendering checks at 160×40 and 60×18 show the original tool,
  native identity and explicit selected-resume action in the existing browser.
  The dashboard composition and pane geometry are unchanged from ticket 11.

Build, formatting, all-target Clippy, filtered bin tests and exact ownership
regressions are recorded with the ticket's completion comment. The older native
recovery fixture was updated to wait for the stable source row before Enter,
matching ticket 11's read-only session browser, and to accept the more specific
unavailable-tool diagnostic. No unfiltered ownership/resource suite or final
physical reboot gate is claimed.
