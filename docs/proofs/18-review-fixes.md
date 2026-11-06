# Milestone 1 final review corrections

2026-11-06, stable Rust 1.99.1/Cargo 1.99.1, macOS. Production checkpoint
`65ad6bbd8ab832f66613d7ba079b870c44860cba` starts from integrated
`ada4809c3785bab575fcdff4b70730f03047fcba`. Its debug application SHA256 is
`ea7995f3db79758b0377f5c1568c253f9cb74577d566c08e48f269fcd2f155ed`.
Historical native and resource artifacts retain their original source boundaries.

## Corrections

- `App` accepts usable viewport dimensions, owns successful dispatch/pending
  policy and retries bounded-queue rejection on a later frame. `Ui` still owns
  geometry; `main` mechanically dispatches the resulting effect.
- `events` owns canonical effects, session requests/observations, launch results
  and lifecycle vocabulary, with compatibility reexports to limit churn.
- One launch-intent helper preserves the existing stable-request policy for
  fixture, Claude, Codex and explicit native resume. Distinct persistence
  prepare/authorize/save commands carry one named metadata snapshot.
- Additive typed task/session/request identity supplies runtime and durable
  session keys. Display labels are derived. Old durable JSON converts at the
  persistence read boundary with bounded lengths and validated session UUIDs;
  task/request IDs remain opaque. Malformed records produce visible errors
  without discarding their metadata or transcripts. Existing schema and wire
  labels are retained.
- Native process outcomes now distinguish successful `Exited` from nonzero or
  signalled `Failed`. The original code/signal is retained before descendant
  cleanup and independently of transient storage/control errors. Explicit Stop
  and quit of running execution remain `Stopped`. Failed sessions drain, flush,
  seal and obey the explicit-next-launch barrier; readable history includes the
  original outcome. Cleaned failures reload without uncertainty or automatic
  restart. Cleanup failure remains separately `StopFailed` and retryable.

## Red/green evidence at the approved seams

The App viewport test first failed because viewport acceptance/result methods
were absent, then passed acceptance, bounded-dispatch failure, retry, deduplication
and unusable-geometry preservation through the actual App interface.

The new real-dashboard/same-executable-supervisor/native-style shell PTY test in
temporary Git checkouts reproduced the old process bug before its fix:

```text
original native outcome must survive descendant cleanup
left: String("Exited")
right: "Failed"
```

The red working tree was based on `ada4809c`, with the new test and the earlier
vocabulary/payload/viewport refactors; native exit classification was unchanged.
The fixed scenario passed code 7, code 0 and SIGTERM cases. Each checks an
intermediate failing shell command while the CLI remains Running, final output
through public History, eligible sealed capture, supervisor death/reload,
retained task/session/native identity, explicit resume with a new session and
the original native conversation, and explicit Stop as Stopped. The code-7 case
also injects history sync ENOSPC: it checks the original outcome survives,
replacement is refused through the canonical launch result, and explicit repair
and retry complete capture. Focused result: passed, 5.46 seconds.

The existing version-one native recovery fixture now additionally verifies
conversion preserves opaque task `task16`, a managed session UUID and opaque
request `prior native16 λ`, before explicit reconciliation/resume. Focused result:
passed, 1.25 seconds. Normal/narrow rendering checks verify `Failed` and the
original exit reason stay visible after a later control error.

An initial complete run exposed three old tests expecting `Exited` after missing
executables. Only those expectations were corrected to `Failed`; gate-exec
coverage also checks the retained nonzero outcome after restart. The gate-exec
focused check passed in 0.96 seconds; both missing-tool checks and missing-checkout
recovery passed together in 3.69 seconds. No production code changed for these
expectation corrections. The initial sandboxed PTY attempt could not acquire its
private socket and is not a behavioral pass; real PTY checks use the authorized
unsandboxed macOS boundary.

## Verification and limits

Build, formatting and warnings-denied all-target Clippy passed on `65ad6bb`.
Its unit checks passed 28 tests; the intentionally ignored subprocess probe was
exercised by the real PTY restoration parent. Final complete-suite results follow
after the test-only missing-executable expectation corrections.

This new outcome regression uses real OS process/PTY/supervisor/persistence
boundaries with a scripted native-style CLI. It does not claim an induced crash
of actual Claude Code or Codex, a new native version, graphical terminal support
or new resource measurements. The [historical integrated native proof](18-milestone-evidence.md)
and [tracker measurements](17-tracker-revalidation.md) remain attributed to their
recorded source and workload. No queue, screen, capture, commit-ring or transcript
chunk budget, tracker cadence, storage algorithm or process ownership policy
changed; the ignored sustained measurements were not repeated for these focused
interaction/data/outcome corrections.
