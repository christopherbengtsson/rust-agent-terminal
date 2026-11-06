# 08: Qualify readable native session history

Status: ready-for-agent
Resolved: yes
Blocked by: 03, 05

**What to build:** Open retained history as useful readable text with stable passage locations, independently of native resume and automatic naming.

- [x] Provide a safe readable history view from committed captured output while retaining raw bytes and identities independently; never execute captured terminal controls.
- [x] Handle UTF-8 and terminal control/redraw sequences across capture/page boundaries with bounded work and stable raw-to-readable passage locations. Explain irreducible terminal artifacts rather than promise structured chat reconstruction.
- [x] Demonstrate recognizable prompts, answers and command output with actual Claude and Codex on the existing adopted-workspace launch path; input is retained only through appropriate visible/native evidence, not arbitrary key/password logging.
- [x] Retain useful readable passages after restart, missing checkout or unavailable native conversation; respect committed extents, deletion tombstones and per-session ownership.
- [x] Record actual native versions and history examples, projection limits and focused fixture cases. This ticket supplies the readable view/projection used by search; global indexing/search scheduling belongs to ticket 11.

## Comments

Verify this path through the established real-state/fixture seam and actual
native captures. Native prompt delivery and naming are qualified in ticket 07.

Qualified in [the readable-history proof](../../../docs/proofs/m2-08-readable-history.md):
bounded committed projection, independent escaped raw action, stable session/raw
passages, cross-page UTF-8/controls, restart/missing-checkout/native-record
independence, deletion and owner/request attribution. Fresh actual Claude Code
2.1.319 and Codex CLI 0.167.0 prompts/answers/completed command-output markers,
public readable pages and history/raw dashboard captures are archived with hashes.
Ticket 06 final source was merged; focused checks pass. Aggregate integration/PTY
gates and the later reboot/release workload remain with the integration runner
and ticket 19, respectively.

Reopened after the combined 06/08 aggregate at `610c723febb759dc76c23bee0f783f96ee679825`:
ownership checks report 61 passed, 12 failed and 2 ignored. Existing raw-view
expectations need verification against the explicit raw action, while pagination,
deletion and recovery failures require separate compatibility corrections.
Native corpus qualification remains limited to its recorded source and versions;
it does not establish a passing aggregate. See [integration evidence](../../../docs/proofs/m2-06-08-integration-gate.md).
Close again only after corrections and the required full compatibility gate pass.


Scoped compatibility repairs closed with code `6e73aa2` and the explicit
[assembled-check record](../../../docs/proofs/m2-06-08-integration-gate.md#scoped-assembled-closure-2026-11-07).
Frozen source `5b3ab57` passed every runtime/history case and failed only clipped
footer help; that aggregate remains failed. The focused footer correction passed
rendered normal/compact checks, the exact cleanup case, all units, previously
skipped scripted/screen targets, and build/fmt/Clippy. The integration decision
accepts this assembled 06/08 closure rather than another capture run solely for
help text. A final merged milestone aggregate/global review remains required;
native qualification stays limited to its recorded tools, sources and workloads.
