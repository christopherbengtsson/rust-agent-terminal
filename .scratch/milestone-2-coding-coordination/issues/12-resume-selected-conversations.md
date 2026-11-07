# 12: Resume the explicitly selected native conversation

Status: ready-for-agent
Resolved: yes
Blocked by: 11

**What to build:** Choose an older or latest conversation in a workspace and explicitly recover it through its original native tool.

- [x] Resume targets retained tool/conversation identity, not current screen or history index; create a new managed history segment under the same workspace and native conversation.
- [x] An occupied worktree offers attach; another conversation requires confirmed stop and settled prior capture. Preserve canonical checkout/uncertain-execution reservations.
- [x] Explain missing checkout, unsupported/unavailable native recovery and missing IDs; preserve readable history and offer an explicit new conversation. Never transfer native state between tools.
- [x] Resume does not rewind Git. Restore/startup/reconciliation never submit a prompt or automatically resume, and same-request recovery never duplicates native execution.
- [x] Verify selected older/newer conversations and tool switches with fixtures and actual Claude/Codex native markers; record exact versions and original-ID evidence.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Completed 2026-11-07. Stable workspace/source-segment selection resolves the
original tool and native UUID through the sole writer, without loading an old
Observation into the owner. Latest uses the same resolver. New managed segments
carry chosen identity before durable Prepare. Schema 9 binds request/source
identity and durable cancellation independently of transcript retention. Lookup
and post-lookup admission preserve setup, canonical checkout, capture and uncertain
execution reservations; occupied work offers attachment and requires settled Stop
before another conversation. Reading/search/navigation never changes launch intent.

Native proof on macOS 26.2: Claude Code 2.1.319 and Codex CLI 0.167.0 each stored
two distinct conversation-only markers in a fresh test workspace. After all exited
and the isolated supervisor restarted with integrated product source `39dfd0d`,
explicit selection order B/A/D/C recovered all four original IDs in fresh managed
segments and returned their distinct markers to prompts containing no marker.
Git HEAD, staged/unstaged diffs and untracked contents remained unchanged. All
observed native leaders and isolated supervisor PIDs were absent after confirmed
shutdown. Claude semantic status remained conservatively Unknown.

Evidence: [selected native recovery](../../../docs/proofs/m2-12-selected-native-resume.md),
[original/native/new-managed IDs, binaries, argv, cwd and cleanup](../../../docs/proofs/m2-12-selected-native-summary.json).
The proof identifies both source/binary runs and keeps real native positive cases
separate from fixture unavailable-record/version failures. No user native records
were deleted or credentials copied.

Validation: `cargo build --locked`, `cargo fmt --all -- --check`,
`cargo clippy --locked --all-targets -- -D warnings`; filtered bin suite 98 passed,
2 ignored, with the two terminal PTY test names excluded. Exact ownership checks
passed: selected original/tool/native source (including pending Prepare, changed
same-request source, reconnect/Stop during lookup, occupied refusal, missing
checkout, native rejection, raw history and Git preservation), Codex original ID
recovery, unavailable recovery, prior-boot uncertainty/reconcile, native process
outcomes, canonical aliases, lost launch acknowledgement and registry alias/restart.
Worker checks passed for missing tool/native ID, transcript deletion, request
binding/cancellation restart. App/render checks show explicit source selection and
action at 160×40 and 60×18. One earlier bin invocation used the wrong skip name;
terminal restoration ran and passed then, with no concurrent native proof or
resource/timing claim. Final ownership/resource and physical reboot gates remain
later-ticket work.
