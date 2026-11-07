# 10: Run configured setup with recoverable failure choices

Status: ready-for-agent
Resolved: yes
Blocked by: 09

**What to build:** New workspaces run the project setup command with visible progress before a requested launch, and retain useful recovery choices if it fails.

- [x] Persist optional project setup configuration and run it as owned execution in the workspace. Show bounded progress, success/failure and cancellation; no configured command means no setup work.
- [x] On failure retain workspace and offer Retry setup or Continue without setup. Explain whether continuing carries out the previous Create and start intent; failure alone never launches coding.
- [x] Keep setup cancellation/Stop/quit responsive, account for pending creation and clean owned processes. A lost or uncertain result cannot automatically replay a command with external effects.
- [x] Use stable operation identity to reconnect to the same result and prevent late completion from launching cancelled work. Setup cannot become an alternate unrestricted future-reviewer path.
- [x] Verify success, failure, cancellation, slow command, storage/transport loss and explicit continuation with actual temporary scripts, while unrelated sessions keep running.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


Implemented in isolated branch `feat/m2-10-setup` from exact `fb60d1a`, including
latest integration `e715dc1`, ticket 11 schema 7 before setup schema 8, and final
product source `343c561`. Setup has durable frozen project policy/attempt receipts,
owned gated execution, bounded honest progress, explicit Retry/Continue effects,
canonical admission/Stop/quit reservations and no restored launch permission.
The creation-to-first-setup receipt gap now restores only a known-never-started
choice with no execution. Duplicate Create/config changes preserve original intent
and policy. Original native outcomes/session/final lifecycle also survive later
conversations and restart through typed read-only metadata, without extra owners.

Scoped evidence: [project setup](../../../docs/proofs/m2-10-project-setup.md).
All 11 setup Controller/actual-PTY cases passed; final original-receipt restart,
no-config creation and v1 real-supervisor migration checks passed. Locked build,
formatting, all-target warnings-denied Clippy and diff check passed. The broader
filtered bin run passed 95 with 2 explicit corpus replays ignored and terminal
PTY tests excluded; final historical presentation changes have the scoped restart
and migration evidence. Real current Claude/Codex setup compatibility, physical
reboot and whole-milestone aggregate qualification remain explicitly unclaimed;
ticket 19 owns the final aggregate. No hidden provider or extra coding turn ran.
