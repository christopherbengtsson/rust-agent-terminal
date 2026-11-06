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
| `storage_saturation_keeps_navigation_retry_stop_and_exact_retained_bytes` | Line 3331, expected zero pending bytes but observed 137188 |
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
