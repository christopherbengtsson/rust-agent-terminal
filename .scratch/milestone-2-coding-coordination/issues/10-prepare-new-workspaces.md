# 10: Run configured setup with recoverable failure choices

Status: ready-for-agent
Resolved: no
Blocked by: 09

**What to build:** New workspaces run the project setup command with visible progress before a requested launch, and retain useful recovery choices if it fails.

- [ ] Persist optional project setup configuration and run it as owned execution in the workspace. Show bounded progress, success/failure and cancellation; no configured command means no setup work.
- [ ] On failure retain workspace and offer Retry setup or Continue without setup. Explain whether continuing carries out the previous Create and start intent; failure alone never launches coding.
- [ ] Keep setup cancellation/Stop/quit responsive, account for pending creation and clean owned processes. A lost or uncertain result cannot automatically replay a command with external effects.
- [ ] Use stable operation identity to reconnect to the same result and prevent late completion from launching cancelled work. Setup cannot become an alternate unrestricted future-reviewer path.
- [ ] Verify success, failure, cancellation, slow command, storage/transport loss and explicit continuation with actual temporary scripts, while unrelated sessions keep running.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.
