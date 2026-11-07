# Actual macOS sleep/wake — 2026-11-07

The user performed Apple-menu Sleep and reported “awake”. macOS power logs
record Software Sleep at 09:49:43 +0100, DarkWake at 09:50:01 and Full Wake
at 09:50:26. This is a real machine transition, not process suspension or an
injected boot fixture. The [sanitized evidence summary](m2-19-sleep-wake.json)
retains those actual events and hashes of the complete private observations.

The reviewed source was `59fed09a8c13505b68d93dd10e5612419a3d08d5`, product SHA-256
`b50530e0e859eb23fd7c61d4442d5fc8dd47c11aaffe39615897be76df460faf`. Both explicitly
prepared original-ID conversations retained the same managed/native IDs and exact
Darwin executable, argv, process-start identity and canonical cwd after wake.
No managed segment or unexpected checkout process appeared. Project/workspace
membership, selected native context, code/index/untracked/ignored witnesses,
recovery refs and all previously committed transcript prefixes verified unchanged.
The same kernel boot UUID/boottime is expected for sleep/wake.

The reboot verifier deliberately exited 1 on the unchanged boot identifiers and
its zero-Running requirement: these two explicitly prepared conversations were
still Running. Every other state check passed, with no unverified checks. This
is not relabelled as a reboot pass. The refreshed immediate pre-reboot checkpoint
is `~/.rat19-transition-59fed09/actual-after-wake-immediate-pre-reboot-01.json`;
exact continuation commands remain in that root's `POST-PHYSICAL-COMMANDS.md`.
Persistent IPC secrets and active hook argv are excluded from tracked evidence.

**Sleep/wake is verified. Physical reboot, C6 and ticket 19 remain open.**
No reboot has been authorized or performed by the agent.
