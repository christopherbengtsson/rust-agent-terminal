# Milestone 2 integrated qualification

Ticket 19 remains **Resolved: no**. This record will distinguish checks completed
on one frozen source/binary set from outstanding gates. Prior feature proofs are
supporting evidence with their original source/tool boundaries; they are not a
new integrated qualification.

## Frozen aggregate procedure

`scripts/m2-19-gate.py prepare <new-external-directory>` runs stable locked build,
format, warnings-denied all-target Clippy and prebuilds every target. After source
is committed and frozen, `run <same-directory>` records the source, clean state
and SHA-256 of every executable before and after the serial locked all-target
`--nocapture` command. Builds, source edits and unrelated native/PTY/resource
workloads must remain outside that interval. An aggregate failure remains a
failure even when a later focused check succeeds.

## Required evidence

- A declared three-project/six-session coding-only fixture workload, visible
  navigation/attach timing, per-stream capture/durability/queue bounds and measured
  process/storage growth during history search, diff and selective storage faults.
- An integrated keyboard workflow with six actual native coding sessions,
  creation/setup/search, original-ID older conversation recovery, both diff modes,
  real Vim/Pico return, checkpoint-backed archive/branch cleanup/Return and
  separate safe folder removal with retained history and local code recovery.
- Current native tool versions, actual prompts/questions/approvals, Ctrl-C,
  resize/focus escape, same-session attach, scoped Stop, quit and reconnect.
- Actual detach, ordinary quit, dashboard loss, supervisor crash, sleep/wake and
  physical reboot recovery with no duplicate or automatically restarted coding.
- Normal 160×40 and compact 60×18 captures compared with the vision image.

## Open external gates

No physical reboot is authorized or performed by this record. The spec requires
explicit user permission before reboot. A durable handoff must identify retained
state, source/binaries, session/native IDs and the exact post-boot checks before
that action is requested. Injected boot fixtures do not satisfy this gate.
Actual sleep/wake remains unverified until its real machine transition is
recorded. Neither case can be claimed from prior feature proofs. C6 and milestone
completion remain unclaimed while a required case is unverified.

This ticket does not qualify GitHub review automation, publishing, task PR
continuation, notifications or the combined release workload.
