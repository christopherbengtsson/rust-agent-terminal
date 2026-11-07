# 19: Complete the Milestone 2 coordination evidence

Status: ready-for-agent
Resolved: no
Blocked by: 12, 15, 18

**What to build:** Demonstrate C1–C7 together with both native agents, editors, concurrent projects and honest recovery/resource evidence on macOS.

- [x] Run the integrated keyboard workflow across three projects and six concurrent coding sessions, including creation/setup/search/resume/diffs/editor/archive/branch cleanup/removal; preserve one active session per worktree.
- [x] Record native versions and actual prompt/approval/Ctrl-C/resize/attach/quit/reconnect behavior for Claude and Codex on final relevant source. Retain original-ID history and unavailable-recovery cases.
- [x] Measure input-to-visible navigation against 100 ms and usable attach against under one second, plus fair per-session durability/queue/memory/disk behavior during searches/diffs and storage faults. Separate fixture/native evidence.
- [ ] Exercise detach, normal quit, dashboard loss, supervisor crash, sleep/wake and real reboot recovery without duplicates/automatic coding restart. Arrange physical reboot explicitly with the user and retain unverified cases as open gates.
- [ ] Run stable locked build, formatting, warnings-denied all-target Clippy and all-target tests. Compare normal/small terminal captures to the vision, align working instructions and report limitations. Do not claim later GitHub/reviewer or notification release gates.

## Comments

Prepared from the agreed Milestone 2 spec and architecture exploration. No
implementation or new validation evidence is claimed by this ticket.


2026-11-07 non-physical qualification checkpoint:

- The [qualification record](../../../docs/proofs/m2-19-qualification.md) retains
  the failed first aggregate, causal regressions/corrections and genuinely clean
  corrected serial all-target pass: 274 passed, seven explicit ignores at
  `335ba44e980f2e448f86bafe85aa4bfac82b1e5d`, with every executable unchanged.
- `a56eee5b25196f236969b5f2051e3d85707f3e0d` changes only the ignored measurement
  and analyzer; the production tree and executable remain identical. Static
  preparation, strengthened six-owner measurement and actual installed-editor
  checks passed again. Final integration review/checks remain separate.
- [Fixture and default-Claude supplement](../../../docs/proofs/m2-19-measurements.md)
  distinguish all-six prepared-owner timings from the first current-owner-only
  run, selective storage failure, partial indexing and 2.1.319 Unknown semantics.
- [Actual integrated native proof](../../../docs/proofs/m2-19-native.md) records
  six concurrent coding sessions across three projects with supported Claude
  2.1.318/Codex 0.167.0, original-ID recall, real editors, archive/branch cleanup/
  Return/removal/local recovery, retained history and distinct unavailable cases.
  Detach, ordinary quit, actual dashboard loss and active-owner crash have
  recorded outcomes; no automatic coding restart was observed. The conservative
  same-boot Interrupted reservation is retained.
- Actual sleep/wake and physical reboot remain **unverified**. No reboot or sleep
  was performed or authorized. The [physical-transition plan](../../../docs/proofs/m2-19-physical-transition-plan.md)
  is explicitly unexecuted, for preparation after review/fixes/final checks.
  User restart authorization is a later explicit step. C6, this ticket and the
  milestone remain unclaimed. No later GitHub/reviewer/notification release gate
  is claimed.
