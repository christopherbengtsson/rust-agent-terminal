# 16: Stop, save and archive a workspace

Status: ready-for-agent
Resolved: yes
Blocked by: 10

**What to build:** Stop and archive saves unfinished code locally, reports exactly what was saved and hides the workspace without deleting its folder or chats.

- [x] Cancel pending launch/setup intent and establish confirmed owned-execution cleanup and settled capture before checkpoint; block new coding admission while mutation is pending.
- [x] Save tracked and new non-ignored changes as a clearly named Archive checkpoint commit; avoid empty commits on clean work. Preserve ignored files and explain exclusions.
- [x] Handle conflicts, submodules/nested state, Git identity/hooks errors and concurrent external edits without false save success, bypassing hooks or silently changing unrelated work/index state.
- [x] Pin saved HEAD with a workspace-specific recovery ref and durable workspace metadata before reporting recovery protection. Lost replies/crashes reconcile prior save instead of duplicating commits.
- [x] Confirm local checkpoint ID/not-pushed coverage and archive visibility separately. Failed saving leaves unarchived; unarchive restores visibility without coding restart. Verify retained code after Git GC and transcript deletion.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


2026-11-07: Implemented on `ticket16-archive`, product/test source `ab268c9`
(implementation `6e454e1`, latest integration `3b4b364` merged at `8f0ffe2`).
Schema 10 retains operation-bound Waiting/Prepared/archive metadata independently
of transcript deletion. Canonical admission gates include asynchronous selected
resume after lookup; Stop cancels relevant native/setup/creation handoffs and
archive waits for actual execution/capture/durable-write settlement. Real-index
locking plus original staged-tree/prepared-tree refs preserve staged-only content;
ordinary Git checkpoint commit and final index normalization avoid false dirty
state. Metadata acknowledgement controls visibility; lost replies and supervisor
loss after Git reconcile the same checkpoint. Unarchive starts no coding.

Scoped evidence: 15 real-Git public Job tests, seven Controller/dashboard scenarios
(together 16.64 seconds), selected-resume ownership regression (4.35 seconds),
App 32, UI 18, selected-resume persistence 1, locked build, formatting and
warnings-denied all-target Clippy passed. See
[archive proof](../../../docs/proofs/m2-16-archive.md) and actual normal/small
PTY captures linked there. The reference-fsync configuration follow-up was
rechecked with all 15 real-Git tests (2.14 seconds) and Clippy.

Scope limits remain explicit: no automatic branch deletion/restoration (17),
no folder removal (18), no full combined native/resource/reboot qualification
(19). Physical power loss/reboot and real Claude/Codex archive compatibility were
not established by these fixtures. Uncertain stale Git locks after abrupt loss
require verified manual repair; they are never silently removed. Ignored files,
Git filters/LFS external resources and unsupported nested/index states are not
claimed as complete backup coverage. Only ticket 16 is resolved here.
