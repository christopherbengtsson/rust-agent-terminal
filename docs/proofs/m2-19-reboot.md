# Actual physical reboot and postboot recovery — 2026-11-07

The user performed the separately arranged physical restart and reported
“rebooted”. Kernel boot UUID changed from
`29BAFBA1-DCA5-4B2B-8CC4-458DC32D50D5` to
`3B5D74F5-B7DE-441B-B4B2-1BE5A773F691`; boottime changed from
2026-09-18 10:11:09 to 2026-11-07 10:06:51. This was an actual machine reboot,
not an injected boot identity or a process restart.

The immediate pre-reboot state had two explicitly resumed idle native sessions,
one archived workspace with folder/branch retained, one removed+archived
workspace, one stopped workspace with tracked/staged/untracked/ignored witnesses,
and the conservative Interrupted reservation, across three projects/six records.
It used reviewed source `59fed09` and the product hash below.

[Sanitized restore evidence](m2-19-reboot-restore.json) records the before/after
boot identities and every check. Initial read-only verification found all durable
state intact and no unexpected checkout process, but correctly left four public
checks unverified while the owner was absent. A separately explicit owner-only
bootstrap then passed **all 18 checks** with no failed or unverified checks:
zero automatic native coding/new segments, original IDs/membership/selected
Violet context restored, and code/index/witnesses/recovery refs/committed prefixes
unchanged. The native recalls below occurred only after this zero-start proof.

The [curated native archive](m2-19-reboot-evidence.tar.gz) and
[12-file hash manifest](m2-19-reboot-manifest.json) contain new reply captures,
explicit action plans, sanitized observations, cleanup confirmation, tool hashes
and preserved summary/curation failures. Full native argv, live invocation tokens,
IPC secrets, raw state, provider profiles and host process inventory remain private.

## Explicit postboot native recall and cleanup

The user-performed real reboot was followed by owner-only restore with zero native starts. This final phase used frozen source `59fed09a8c13505b68d93dd10e5612419a3d08d5` and product SHA-256 `b50530e0e859eb23fd7c61d4442d5fc8dd47c11aaffe39615897be76df460faf` on actual macOS, through proof-only PATH Codex 0.167.0 and Claude Code 2.1.318. No build or repository edit occurred.

| Tool | Exact selected previous managed segment | New managed segment | Original native conversation |
| --- | --- | --- | --- |
| Codex 0.167.0 | `dedec21c-e0ad-4b70-a862-29d6d9ea2fa0` | `19170340-9663-4c87-a2ed-3946ddd54a12` | `01a1b3e1-41ae-74e0-952b-8af0aaeb7a84` |
| Claude Code 2.1.318 | `1e66e7de-3d06-47d7-935c-395d2d2566a4` | `2769e9e3-c70d-4b76-b838-9a5a79677add` | `dcfb6eda-f2a9-41fc-ad34-333ba1067577` |

Each exact historical segment/native UUID was visibly verified in the filtered retained-conversation browser before explicit Resume. No initial prompt was supplied by the launch. The new prompt excluded the old marker and forbade tools and file access. Fresh replies were `POSTBOOT_RECALL: M2_19_AMBER_CODEX_RECOVERED` and `POSTBOOT_RECALL: M2_19_VIOLET_CLAUDE`. Both had current-turn Completed evidence, bounded to 30 seconds; silence and later Unknown are not completion proof. Codex invocation-local hooks were reviewed/trusted through its native Hooks UI. No global permission or authentication change occurred.

Fresh Darwin executable, exact raw native argv, process start and canonical cwd checks passed for each native leader. The actual current owner was freshly authenticated using Unix LOCAL_PEERPID and its exact canonical executable/argv/start. Its current ancestry contained the expected native trees. Complete raw native argv remains private because Codex hook arguments contain an invocation-local callback token. The shareable summary retains resume operands, executable, identity and full private argv hash, excluding that token.

New boot did not automatically clear Interrupted reservations. Existing explicit dashboard `u` reconciled Amber and Violet before their resumes; the same control later released prior Cobalt's reservation without starting it. Cobalt remains historically Interrupted, now unreserved. No saved PID was signalled and no database reservation was manually cleared.

Both resumed sessions were explicitly Stopped through product controls. Capture, metadata and registry settled; authenticated ordinary Shutdown then confirmed stopped. Every process in the freshly attributed live owner/native ancestry disappeared. Supervisor and native-status endpoints were absent. The final cold snapshot exactly preserves preboot code/index/untracked/ignored witnesses and typed recovery refs, all old identity bindings and committed transcript prefixes, and adds exactly the two expected new managed segments and workspace bindings. Removed AmberClaude, archived VioletCodex and stopped CobaltCodex remain preserved.

The outer PTY driver's post-exit termios observation returned ENOTTY (`terminal_restored=null`); this phase makes no terminal-restoration claim. The first summary-only check incorrectly demanded complete membership equality despite the two intended new segments; that failed assertion is retained privately and corrected by checking every old binding plus exactly the two expected additions. No native runtime attempt failed or was rerun.

Only `postboot-shareable/` and the paths in its manifest are curated for publication. Raw state, IPC authentication, provider profiles, full host process inventory, and native raw argv remain private. The root agent owns repository documentation and tracker acceptance.

Actual sleep/wake is recorded separately in [its proof](m2-19-sleep-wake.md).
Together these close the required physical C6 gates for this declared macOS
source/tool/workload boundary. They do not qualify later GitHub/reviewer/
notification release gates, power-loss durability or unsupported native versions.

After final capture and cold audit, both clean temporary proof-source Git
worktrees (`a56eee5` and `59fed09`) were removed. Their exact commit objects
remain ancestors of the integration branch; pinned executables/private state
and curated evidence are retained. Historical pin files are unchanged. Recreate
a detached source worktree at its recorded commit before rerunning any old
helper that requires the historical source path; cleanup is not a new runtime
qualification or a rewrite of an earlier source/binary boundary.
