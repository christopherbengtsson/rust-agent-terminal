# Combined tickets 06/08 integration gate

On 2026-11-06, the integration branch merged final ticket 06 runtime
`9a1dd26f15386729f81e1b6119f0b7c4903e1883` and final ticket 08
`95a6a97ae808e0f3e54e8f21f7a722adaadbeb69`. The tested combined source was
`610c723febb759dc76c23bee0f783f96ee679825`. Source remained unchanged throughout
the gate; ticket 13 was merged only after the command exited.

`cargo build --locked`, `cargo fmt --all -- --check`, and
`cargo clippy --locked --all-targets -- -D warnings` passed.
`cargo test --locked --all-targets` exited 101. Unit tests passed 55, with 2
ignored. Ownership tests passed 61, failed 12, and ignored 2 in 219.05 seconds.
Cargo stopped at that failing test binary; later integration binaries were not
run by this command.

The failing ownership cases and original source line numbers were:

| Case | Failure |
| --- | --- |
| `bounded_history_pages_recover_every_record_across_multiple_chunks` | Line 1511, page polling deadline |
| `chronological_history_retains_erased_and_alternate_screen_text_safely` | Line 91, missing erased text in the readable redraw view |
| `cleanup_failures_remain_incomplete_until_directory_sync_and_completion_commit` | Line 4569, cleanup condition absent; error reports selected session has no retained transcript while deletion remains incomplete |
| `lost_failed_launch_result_is_recovered_without_executing_after_adoption` | Line 1074, adoption error unexpectedly non-null |
| `oversized_osc_is_captured_without_growing_live_screen_history` | Line 2726, deadline |
| `populated_legacy_registry_migration_preserves_opaque_membership_paths_and_history` | Line 953, missing file |
| `retained_registry_reuses_workspace_across_adoption_alias_and_restart` | Line 5806, retained task identity mismatch |
| `stopped_history_survives_restart_and_missing_checkout_without_launching` | Line 91, old title absent; retained text visible under the new readable title |
| `storage_abrupt_loss_preserves_durable_prefix_and_excludes_unsynced_tail` | Line 3517, deadline |
| `storage_saturation_keeps_navigation_retry_stop_and_exact_retained_bytes` | Line 3331, retained transcript length 137188 differs from reported accepted bytes 0 |
| `surviving_native_execution_blocks_replacement_then_different_boot_reconciles` | Line 1773, deadline |
| `sustained_capture_crosses_caps_while_attached_focused_detached_and_lagged` | Line 2488, deadline |

These are observed failures, not established root causes. Raw-history UI
expectations do not explain all failures. No assertions were removed, no tests
were skipped, and no aggregate pass is claimed. Exact complete output is retained
for the current local correction effort at `/tmp/rat-m2-06-08-gate-full.log`, with
the complete panic subsection at `/tmp/rat-m2-06-08-gate-failures.log`; these local
temporary files are not durable repository artifacts.

After the gate, complete ticket 13 `f84ef929c5e71c8300158d5cf098815fbd549478`
merged cleanly at `283808ac52e54f0db54e9ebd047312f0b3e9fe01`. Build, formatting,
and warnings-denied all-target Clippy passed again. All five Working diff unit
tests passed, and both focused Working diff ownership checks passed serially
(5.11 seconds), including actual normal/small dashboard interaction and bounded
execution-side cancellation. This scoped verification does not replace the failed
combined gate. Tickets 06 and 08 remain reopened pending corrections and a passing
full gate. Ticket 13 retains its scoped qualification. Native support claims remain
limited to the source/tool/workload boundaries in each feature proof; milestone
and release acceptance are not established here.

## Compatibility corrections and the first repair gate

The isolated repair branch began at ticket 13 integration `283808a` and merged
the reopened-tracker integration `5c1a604`. Source commit `f81537a` repaired the
observed boundaries without removing the original assertions:

- Identical raw-history polls coalesce instead of replacing their request UUID
  and clearing the response before a Controller can observe it. A different
  owner, session or passage still rejects the earlier response.
- Workspace selection has a private UUID-attributed completion. Adoption remains
  metadata-pending through that exact receipt, independently of earlier metadata
  commits and registry snapshots.
- Failed launch outcomes are found across owners after adoption. The existing
  lost-failure test now waits for adoption to finish before replaying each of
  fixture, Claude and Codex launch requests; removing the lookup produces a new
  successful child identity instead of the original failed/null outcome.
- Legacy restoration validates identities retained solely in the durable identity
  column, using the same bounded conversion as migration. It does not discard or
  rewrite the original history merely to restore it.
- Final capture counters freeze while the successful seal retires its live
  registration. Holding the exact seal receipt reproduced accepted bytes 0 against
  100 actual bytes before this guard. The regression checks the held receipt,
  completion, reconnect, supervisor restart, a replacement session's separate
  counters and the unchanged earlier transcript.
- The erased/alternate-screen assertion explicitly selects `v` escaped raw after
  `h`; all escaped-control and chronology assertions remain. The readable history
  title retains the existing “Retained history” wording, and a selected tombstone
  still reports “No retained session” with its explicit identity scope.

The first repair aggregate tested product source `0104e03`. A regular `cargo build`
ran after the aggregate began and rebuilt the product artifact under the same
source; a single unchanged binary for the entire run was not established.
The later recorded product SHA-256 was
`2d919defdc5b8f71656a54de67818eecd9b579fcc851a58228a5adf09e73bfce`.
Build, format and warnings-denied all-target Clippy passed. The all-target test
command exited 101: units 61 passed/2 ignored; ownership 72 passed/4 failed/2 ignored
in 477.13 seconds. Every original failing case except the stopped-history readable
display passed, including full numbered-record pagination under sustained capture.
The four remaining failures were
`abrupt_supervisor_loss_recovers_acknowledged_prefix_without_automatic_coding`,
`codex_native_identity_survives_exit_and_resume_uses_original_id_without_new_prompt`,
`failed_final_storage_commit_keeps_quit_visible_until_explicit_retry`, and
`stopped_history_survives_restart_and_missing_checkout_without_launching`.
Each showed a readable page left preparing. Exact output is in the local temporary
file `/tmp/rat-coordination-compat-fullgate.log`; later integration binaries did
not run after this ownership failure.

That run exposed an error in the first repair: raw/preparing and final readable
replies are two stages of one request. Commit `c20cd49` retains the UUID until
final success or error. Its focused staged-success and staged-error tests both
went red before the correction and passed afterward; the tests also preserve
changed-query/owner rejection, clear a failed preparing page, permit explicit
retry, and refresh the same passage to a newer committed prefix. A further
no-child launch replay test went red when querying another owner's cached result
cleared the selected owner's provisional error. Replay now checks that reservation
before clearing its diagnostic and preserves its request/result/control identity.
Cached replies are passed through the existing launch handler without duplicating
other owners' outcome caches. All 63 active unit tests passed with 2 ignored;
build, format and warnings-denied all-target Clippy passed for this checkpoint.

That recorded first-repair product hash stayed unchanged while those additional
unit tests were developed in the separate test harness, after the earlier regular
build. The source-fixed aggregate result
remains failed. The corrected full aggregate and final integration review are
still required; no new native-tool or milestone-completion claim follows from
these fixture and state checks.

Before the corrected aggregate, the four failed readable cases were rerun through
their original PTY seam. Abrupt-prefix recovery, Codex history/native identity and
final-storage retry passed (6.08, 3.84 and 2.63 seconds). Stopped-history recovery
then reached its later escaped bracketed-paste assertion, which correctly failed
in the default readable view. The test now verifies retained readable text first,
explicitly selects `v` escaped raw, and keeps that exact escape assertion. The
corrected stopped-history test passed in 2.55 seconds. This preserves both view
contracts; no raw content assertion was removed or weakened.

## Frozen corrected gate and the remaining status fixture

The next aggregate prebuilt every target with `cargo test --locked --all-targets
--no-run`, recorded source `06900269e7eca8a84b6370d12a066aca4628f3a9` and every
test/product executable hash, then ran `cargo test --locked --all-targets --
--test-threads=1`. Source stayed clean and unchanged, and every recorded binary
hash matched afterward. The product SHA-256 was
`b998149c5c56ed287c7e3db2071d18715d322c0f900d1078f13fcd992c892c10`.
The source-specific local log is
`/tmp/rat-coordination-compat-0690026-all-targets-serial.log` and ends with
`boundary_stable: true`.

Units passed 63 with 2 ignored. Ownership passed 75, failed 1 and ignored 2 in
447.63 seconds. All twelve originally failing cases passed, including every
numbered record from the sustained four-run capture. The command still exited
101 before later integration binaries. Its sole failure was
`codex_status_requires_current_attributed_evidence_despite_liveness_and_prose`:
the fixture never received the quoted prose, the dashboard reported its bounded
32-request transport queue was full, and semantic status correctly stayed Unknown.
This was not a native-evidence classification failure.

The original 57-byte unpaced key burst passed 25 subsequent isolated runs, so the
natural race was not represented as deterministically reproducible. A temporary
fixture-only causal probe paused its live authenticated supervisor for 500 ms
while the dashboard consumed that same burst and its outer output was drained.
That reproduced the exact queue-full/missing-echo failure. The live owner was
resumed by an independent bounded thread; no restored PID or unrelated process
was signalled. An earlier 200 ms probe that joined before draining output passed
and did not establish reproduction. Probe-only code was removed afterward.

For the semantic-negative fixture, the same prose now arrives through one real
outer-terminal bracketed-paste event, followed by Return. The scripted child
echoes escape wrappers literally, so its existing `paste-off` command first
disables child-side wrappers; the dashboard still takes its actual paste event
and native delivery path. The original received-prefix, Unknown/Running,
malformed-receipt and failed-command assertions remain, and a complete prose echo
assertion was added. A paste probe with child wrappers still enabled delivered
the complete payload but failed the unchanged plain-prefix assertion; this was
recorded separately, not called a pass. Under the same 500 ms consumer delay,
plain-child-mode raw keys still failed and the real paste passed all assertions
in 3.23 seconds. The final fixture without the probe passed in 2.79 seconds.

No status classification, evidence freshness/version bounds, queue capacity or
input-saturation/Stop test changed. The observed raw-key burst limitation remains:
when individual key requests outrun the bounded transport, the UI reports queue
full and requires recovery; this fixture correction does not qualify arbitrary
unpaced input delivery. Probe logs are local temporary files under
`/tmp/rat-status-0690026-*`; the final focused run is
`/tmp/rat-status-paste-final-focused.log`. A new frozen full aggregate is required
after the fixture correction before claiming an aggregate pass.
