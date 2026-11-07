# Integrated native qualification — ticket 19

Fresh macOS keyboard/outer-PTY evidence used clean source
`a56eee5b25196f236969b5f2051e3d85707f3e0d` and copied product SHA-256
`89382b1a23124461fbec2bd0944e4b012cfb8d7f056990119a6a02a777569154`.
The persistent detached source is `/Users/christopherbengtsson/.rat19-source-a56eee5`;
private native state/tooling are at `/Users/christopherbengtsson/.rat19-proof-a56eee5`.
`pin.json` and `manifest.json` bind source, executable, tools and parser. The main
run used supported Claude Code **2.1.318**, isolated through a proof-only PATH,
and Codex **0.167.0**. The [separate default-Claude supplement](m2-19-measurements.md)
qualifies actual 2.1.319 interactions with semantic status Unknown. No monitor
version was widened. These native runs make no fixture-performance claim.

## Three projects and six owners — C1/C2

Keyboard registration added real amber/violet/cobalt Git repositories. `N`
created six distinct canonical worktrees, each with its own branch, configured
setup and actual first-prompt response. The following snapshot records the six
attributable owners, all concurrently Running. `six-known-native-identities.json`
retains exact cwd, argv and process-start observations; these saved PIDs confer
no later signal authority.

| Workspace | Workspace UUID | Original attributable managed segment | Native conversation UUID |
| --- | --- | --- | --- |
| amber-claude | `803ff645-4dbb-49fc-8fa4-275f108f530a` | `7164c02d-e496-40d1-b0d4-e21ec1a92462` | `be6e219c-edbb-4bb2-8ca7-8f79f72879e7` |
| amber-codex | `2ff443dd-42d0-4738-a0cd-46e326ede1ec` | `25cb14ab-b960-4cd0-8b8e-b4aa2c5a7fe8` | `01a1b3e1-41ae-74e0-952b-8af0aaeb7a84` |
| violet-claude | `e75305f8-eee4-4a33-b513-2a512a8440a8` | `a46df576-94d9-4c7c-a4e2-9539bc98cb3f` | `dcfb6eda-f2a9-41fc-ad34-333ba1067577` |
| violet-codex | `8d276c7e-a829-49b3-a6a0-e50946fa0004` | `f34cd3fb-079e-4607-8ae5-61570baf65ad` | `01a1b3e0-24f2-7a41-9c1f-bfa0120d50a6` |
| cobalt-claude | `1d20b2f9-f547-48ec-bb75-f448d5915132` | `72a1834c-bccb-416d-951a-5de87b574b41` | `369c0ea0-d7d4-4a5d-902c-ebadf61381ea` |
| cobalt-codex | `c086d1f7-4c4b-4a68-8e91-780c81e9b364` | `57915b1a-f040-4cb9-bea9-aeafa6549f72` | `01a1b3e0-b2bd-7650-aeef-a3688b9fe6bc` |

The first Claude trust choice was accidentally declined; its failed segment and
native ID remain retained. Explicit same-workspace recovery succeeded. The first
Amber Codex Review-hooks path entered a real conversation without a trusted
SessionStart callback, leaving its native ID unavailable; later trust did not
backfill it. Explicit new conversation with trust before startup captured a known
ID. Transcript prose was never used to invent identity. A second launch in an
occupied canonical worktree visibly refused, preserving exactly six owners and
no seventh runtime.

## Interaction and recovery — C3/C5

Both tools had actual fresh Working, Input/question, Approval/permission,
Completed and keyboard Interrupted observations. Claude's model-generated question
about approval is separately Input; only the later real Bash PermissionRequest
modal counts as Approval. Codex request_user_input and command permission were
also observed separately. Ctrl-C was sent after current parsed cells showed
working, and retained the same native process/managed/native identities.
Normal 160×40 and compact 60×18 resizing, Ctrl-] dashboard focus and same-runtime
reattachment were exercised. Each tool executed an owned foreground heartbeat
before scoped Stop: its heartbeat and native descendants disappeared while the
other five exact managed IDs remained Running. `claude-scoped-stop.json` and
`codex-scoped-stop.json` retain these assertions.

Original successful Claude managed segment
`7164c02d-e496-40d1-b0d4-e21ec1a92462` was selected after a **newer Codex**
conversation in that workspace. `v` resumed native UUID
`be6e219c-edbb-4bb2-8ca7-8f79f72879e7` with fresh managed segment
`83b44932-a7e4-42af-9d2d-c2a250e2086c`, no initial prompt, and actual recall of
the original marker. The reverse case selected original Violet Codex segment
`f34cd3fb-079e-4607-8ae5-61570baf65ad` after newer Claude; fresh managed segment
`de52f61c-875b-48ef-8f86-4aeb6a85ece5` resumed its original native ID and recalled
the original marker with fresh Completed evidence. Both actual argv and retained
original identities are recorded. A resumed Claude observation was Unknown with
“response stop observed (continuation unconfirmed)” and stale evidence despite
SessionStart/UserPromptSubmit/Stop callbacks; that observation is not a semantic
positive.

Unavailable cases remain distinct. Unbound original Codex segment
`c89c6420-5a76-40e2-b04d-aa234dc76b89` produced the visible refusal “Native resume
unavailable: original native conversation UUID is missing; readable history
retained.” Separately, removed workspace `803ff645-4dbb-49fc-8fa4-275f108f530a`
was filtered by original successful managed segment, its known `be6e219c-...`
native ID visibly asserted, and `v` refused: “Workspace archive owns this checkout
or folder removal is pending; settle its receipt before Start”. No new segment or
coding process started. This is the actual archived/removal admission reason,
not a missing-native-ID or deeper missing-checkout-preflight claim. Keyboard Find
`amber` and exact-word retained history search after removal opened a committed
original marker passage (`retained-search-closeout.json`).

## Comparisons and real editors — C4

Amber Claude had earlier committed work, staged/unstaged overlap, new files and
an ignored file. Independent Git base/index/HEAD/file witnesses verified Working
versus Whole: earlier branch commits appeared only in Whole. Selecting `f`
inside the retained workspace picker targeted the exact workspace; adopting a
native pane alone did not change the independent sidebar candidate.

Real Vim and Pico inspected both comparison modes, resized/interrupted and
returned with terminal-settings/context checks. An early source-save attempt
sent keys before a real editor owned the foreground; it instead triggered normal
dashboard quit. It saved no source and is unclaimed. All six owners then durably
Stopped with no private native survivors. Later guarded source saves resumed only
one needed Claude owner, leaving five Stopped; actual source bytes changed while
the staged index stayed unchanged. Both failures and corrected evidence remain.

A separate strict supplement followed the main interval. Real Vim/Pico saves in
existing Cobalt Codex checkout verified exact executable through Darwin
`proc_pidpath`, exact full source-file operand through `KERN_PROCARGS2`, foreground
ownership, unchanged index, terminal settings and return context, with **zero
coding launches**. `strict-editor-supplement.json` pins runner SHA-256
`6a63568701348b7857c68983120195fe279db750df0a815416e45c0250fefaf9`.
The earlier `ps` substring guard is not retroactively called exact API identity.

## Archive, Return, removal and code retention — C7

Actual archive of workspace `803ff645-4dbb-49fc-8fa4-275f108f530a` saved checkpoint
`4823662f7e6c5f7b1bac8aa4d62f489d435e6da2`, with first archive reference
`refs/rat/archive/803ff645-4dbb-49fc-8fa4-275f108f530a/ba22cfad-53bc-4e0d-83c1-19b95d3f5296`
and a staged-only `-index` tree. It cleaned the owned branch, retained the folder
and excluded the ignored file. Explicit Return restored the same branch/code
without starting coding. A second clean archive again deleted that branch.

Removal blocked on real ignored data; that file was explicitly moved to
`evidence/retained-ignored-native.txt`. After local-only acknowledgement, the
folder was removed. With both old branch and folder absent, reflog expiry and Git
garbage collection preserved the removal reference. An independent Git recovery
worktree recovered exact saved code, including all three edit markers; the first
archive's staged-only tree remained readable. Original main HEAD/index/content
were unchanged. `archive-return-assertions.json` and
`removal-code-recovery-assertions.json` retain the assertions. Retained history and
the distinct resume-unavailable cases above survived removal.

## Lifecycle and open physical gates — C6 remains unclaimed

Actual detach, ordinary quit and dashboard loss/reconnect were exercised.
`dashboard-loss-reconnect.json` proves one explicitly resumed Cobalt Claude owner
survived dashboard loss and reattached with identical managed/native/execution.
A current live supervisor Popen was then killed while Cobalt Claude managed
`ebe8ec5a-f329-4ff3-8eb6-a19a849db98b`, native
`369c0ea0-d7d4-4a5d-902c-ebadf61381ea`, was genuinely Running.
`owned-supervisor-crash-1794022703039894000.json` retains that boundary. Explicit
owner restart observed zero automatic Running coding; fresh canonical-cwd audits
found no native/orphan descendants.

Same-boot restoration conservatively retains Interrupted/reserved=true for that
segment. No attempt cleared or signalled its restored PID. Authenticated Shutdown
could not complete while reserved; the zero-coding owner was freshly attributed
through authenticated Unix LOCAL_PEERPID plus exact executable/argv/cwd/start
identity before termination (`cold-current-peer-owner-cleanup.json`). Ordinary
quit with six live owners had separately succeeded. The strict-editor supplement
also closed cold and launched no coding. Final cold audits and checkpoint files
are retained in the archive. **Actual sleep/wake and physical reboot have not
run.** C6 and ticket 19 remain open; boot fixtures do not satisfy them.

## Evidence limits and provenance

The [native archive](m2-19-native-evidence.tar.gz) retains used immutable runner
variants, every plan and raw/parsed-cell capture, successful and failed attempts,
source/product/tool/parser hashes and the durable 15-segment state. Initial
creation provenance and `harness-variants.jsonl`/`harness-failures.jsonl` preserve
adaptations rather than overwriting failed runs. These are parsed PTY screens;
terminal restoration queried after the outer leader exits returns ENOTTY and is
unavailable, not passed. Actual editor return compares live terminal settings.
Early dashboards inherited NO_COLOR; later per-process color enabled cyan without
changing existing native child environments. Long path/header clipping is recorded
in the [visual review](m2-19-qualification.md#visual-review-and-working-instructions).

No credentials were changed and no native history was deleted. Proof-only PATH
and normal scoped native trust used existing credentials; native tools created
their own conversation/trust records normally. The [qualification record](m2-19-qualification.md)
keeps clean aggregate, fixture and native boundaries separate. The
[physical-transition plan](m2-19-physical-transition-plan.md) is explicitly
unexecuted and must be refreshed after final review/fixes/checks before any user
restart request. Later GitHub/reviewer/notification release gates are unclaimed.

Final reviewed verifier
`47cec00b410c74fc3745a4a8c645170a8c7c0e66dfe167382348eacf32a1db01`
captured the cold state successfully (`final-reviewed-v2-cold-capture.json`).
Same-boot verification exited 1 as required: actual boot UUID/boottime remained
unchanged. All original identity/native-ID/code/reference/durable-prefix and
no-new-segment checks passed; fresh runtime audit found no owned process. Public
restoration/context checks are unverified because the owner is absent, not
silently passed. The [handoff summary](m2-19-native-handoff.json) and
[900-file manifest](m2-19-native-manifest.json) retain these boundaries. The native
archive is 8,263,056 bytes, SHA-256
`1325e1781a3cb1782424136b24748a198e133785f767059698d8d4f31c249d77`.
It includes only owned proof/state/tooling data; no external native
credential/configuration trees are packaged.

The original private archive remains unchanged. The tracked copy excludes the
product/parser executables and empty runtime lock; their pins and runner/parser
source remain recorded, and actual executables stay in the private persistent
root for the later physical gate.
