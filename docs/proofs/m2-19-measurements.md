# Ticket 19 measurements and Claude Code 2.1.319 supplement

Two coding-fixture measurements and a separate native Claude Code 2.1.319 smoke provide the scoped evidence below. The [main native workflow](m2-19-native.md) separately records supported Claude 2.1.318 and Codex 0.167.0. Ticket 19 remains unresolved; real sleep/wake, physical reboot and final integration checks remain open. No later release gate is claimed.

## Source and qualification boundaries

Both measurements used macOS 26.2 (25C56), arm64, stable rustc 1.99.1 (58cf8f9c3), Cargo 1.99.1 and Git 2.50.0, with the debug-profile product. Measurements ran serially in the exclusive fixture runtime lane. Native smoke ran later, explicitly in parallel with the separate main native proof; it provides no performance measurements.

| Evidence | Exact clean source | Scope | Result |
| --- | --- | --- | --- |
| Original fixture run | `335ba44e980f2e448f86bafe85aa4bfac82b1e5d` | Three projects/six streams; 15 same-current-owner-0 attaches | Passed, 69.470 s |
| Strengthened fixture run | `a56eee5b25196f236969b5f2051e3d85707f3e0d` | Three projects/six streams; three UUID-verified prepared attaches per owner, 18 total | Passed, 74.302 s |
| 319 behavior smoke | `a56eee5b25196f236969b5f2051e3d85707f3e0d` | One native conversation, sequential launch/resume segments; no timing claim | Scoped behaviors demonstrated; two failed harness attempts retained |

The production `src` tree is identical at both fixture commits: `33cc9a17fffb6b72b4ede4dc537f377d6be58bd6`. The product executable SHA-256 is unchanged throughout: `89382b1a23124461fbec2bd0944e4b012cfb8d7f056990119a6a02a777569154`. The strengthened commit changes only the ignored measurement fixture and analyzer. Its ownership test executable SHA-256 is `300da8963c71ac162d8bb471d2d1534040f25fc3d2ff8d06772e75e4cdf6cefd`; the original ownership test executable is `2dd3a346fa99ce6ed3d205767d0c91ce31ae77ea119e77fc72efaf3e5e98149c`.

Each run records clean HEAD and all six prebuilt product/test executable hashes before and after, with `boundary_stable: true`. No builds, source edits, or competing runtime workloads occurred inside either measurement interval. The complete inventories are in each bundle’s sibling `*-before.json` and `*-after.json`, rather than inferred from a later binary build.

Stable locked build, format check, warnings-denied all-target Clippy and all-target prebuild passed at both sources. At **335ba44 only**, the frozen command `cargo +stable test --locked --all-targets -- --test-threads=1 --nocapture` passed in 792.733 s: main 157 passed/3 ignored; editor 1 passed/1 ignored; ownership 113 passed/3 ignored; scripted interaction 1 passed; screen 2 passed. All six executable hashes remained stable. [Full gate results](m2-19-final-static-evidence.tar.gz) and [complete source-pinned log](m2-19-final-static-evidence.tar.gz) retain that exact interval. The all-target suite was not rerun immediately for the later ignored-harness-only change; a final integration/review suite remains separate. Earlier failed gates stay failed and are not relabeled by this pass.

## Declared fixture workload and observation method

Three independent real temporary Git repositories each had two canonical checkouts, with exactly six distinct managed coding-fixture sessions and six child processes. Each offered 65,536 bytes/s in finite 1,024-byte records containing cursor overwrite/erase, green SGR, lambda/CJK UTF-8 and X padding. The producer lifetime was capped at 120,000 ms; explicit Stop ended these shorter measurements. Offered rate is a workload parameter, not measured completed throughput or native-agent capacity.

The healthy Controller phase lasted at least 30 seconds and repeatedly selected every owner while issuing real retained-history searches and actual Git Working comparisons. A later session-scoped debug ENOSPC write fault affected owner 0 only; another approximately eight-second Controller phase issued searches and comparisons while all six streams continued or backpressured. Outer dashboard navigation and attach timing occupied separate UI phases of the same six-stream workload. They are not claimed as timings while the Controller simultaneously held a pending query/diff; the single controlling connection is released between Controller and dashboard phases. Derivative indexing/capture continued during UI phases.

The timed endpoint is the continuously drained, vt100-parsed visible outer terminal at 160×40, not request enqueue or a focus label. The harness keeps one bounded latest screen, polls it at 1 ms, and timestamps keyboard injection and observed visible cells with the monotonic clock. Navigation measures candidate `j`/`k` selection detail changing between `main` and `other19`. Healthy attach also requires a fresh unique visible `ACK17 m219-NNN` after the same 15-byte `ack17 m219-NNN
` native-input payload. No graphical terminal/font/pixel latency is claimed.

The original run’s candidate selection moved to `other19`, but Enter attached the current adopted session, owner 0. An independent audit found the exact owner-0 UUID in every ACK screen; it found no owner-1 UUID. That run therefore proves current-owner-0 reattachment only. This is consistent with candidate navigation and current-pane attachment being distinct actions; it is not evidence of a product misroute.

The strengthened permanent fixture explicitly selects each retained owner using the public Controller before opening a dashboard for that exact checkout. It verifies the actual managed UUID before the timed Enter, then asserts that the parsed ACK screen still contains the intended UUID. There are exactly three samples per owner, all six owners covered. Controller selection and dashboard startup are outside the prepared-session attach interval. All 18 identity checks passed; the faulted-owner screen also verified owner 0 independently.

## Input-to-visible distributions

| Run | Observation | n | p50 ms | p95 ms | p99 ms | Worst ms |
| --- | --- | --- | --- | --- | --- | --- |
| 335 current owner 0 | Healthy navigation | 60 | 4.785 | 8.742 | 10.283 | 10.283 |
| 335 current owner 0 | Navigation with owner-0 fault | 20 | 4.147 | 4.519 | 4.627 | 4.627 |
| 335 current owner 0 | Healthy visible-ACK attach | 15 | 842.775 | 845.952 | 845.952 | 845.952 |
| 335 current owner 0 | Healthy search result | 108 | 62.554 | 180.150 | 240.990 | 243.857 |
| 335 current owner 0 | Search during owner-0 fault | 24 | 65.279 | 76.652 | 136.865 | 136.865 |
| 335 current owner 0 | Healthy real Git diff | 108 | 132.123 | 147.240 | 159.208 | 160.795 |
| 335 current owner 0 | Real Git diff during owner-0 fault | 24 | 155.023 | 161.379 | 161.518 | 161.518 |
| 335 current owner 0 | Visible Stop for faulted owner 0 | 1 | 171.263 | 171.263 | 171.263 | 171.263 |
| a56 all six prepared owners | Healthy navigation | 60 | 4.840 | 9.227 | 10.049 | 10.049 |
| a56 all six prepared owners | Navigation with owner-0 fault | 20 | 4.152 | 4.355 | 4.641 | 4.641 |
| a56 all six prepared owners | Healthy visible-ACK attach | 18 | 844.011 | 950.150 | 950.150 | 950.150 |
| a56 all six prepared owners | Healthy search result | 102 | 81.220 | 177.013 | 213.338 | 253.191 |
| a56 all six prepared owners | Search during owner-0 fault | 24 | 66.748 | 108.184 | 108.684 | 108.684 |
| a56 all six prepared owners | Healthy real Git diff | 102 | 131.089 | 148.282 | 153.495 | 158.288 |
| a56 all six prepared owners | Real Git diff during owner-0 fault | 24 | 157.068 | 168.597 | 171.000 | 171.000 |
| a56 all six prepared owners | Visible Stop for faulted owner 0 | 1 | 170.344 | 170.344 | 170.344 | 170.344 |

All 60 healthy and 20 fault navigation samples in each run were below 100 ms. All healthy attach samples were below one second: 15/15 for original current owner 0 and 18/18 across the six explicitly prepared owners. The strengthened worst attach was 950.150 ms, leaving a narrow measured margin; this is a declared-workload result, not a host-independent bound.

Each run’s owner-0 fault attach had no visible ACK within the two-second observation window and is right-censored, not a healthy attach pass. Its parsed screen still identified the intended faulted managed session. Visible native output can freeze behind backpressure even when dashboard navigation and Stop remain available. Stop was separately observed at 171.263 ms originally and 170.344 ms in the strengthened run. Repair plus explicit Retry settled retained output afterward; no healthy one-second durability claim applies to owner 0 during the fault.

## Per-stream capture, durability and index state

The rows below report final per-managed-session accepted, written and durable raw byte counters. Each final queue was empty and each sorted retained transcript extent equaled its reported durable prefix. These are raw durability/extent checks, not a complete producer-emission ledger claim: all fixture children share a diagnostic producer JSONL, which was deliberately not parsed as authoritative exact emission evidence. Every durable receipt sequence from 1 through the final observed sequence was retained without a gap. Healthy cadence excludes pre-stream idle startup; owner 0’s healthy window ends at the exact pre-fault receipt, while owners 1–5 include the peer-fault interval.

### 335 current-owner run

| Owner | Accepted B | Written B | Durable B | Pending B | Capture HWM B | Backpressure count | Subscriber HWM B | Receipts | Max healthy age ms | Max healthy gap ms | Indexed frontier B | Passages |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 2754581 | 2754581 | 2754581 | 0 | 70717 | 3 | 62035 | 189 | 453.774 | 386.975 | 220771 | 434 |
| 1 | 3318878 | 3318878 | 3318878 | 0 | 13312 | 0 | 63517 | 234 | 482.609 | 392.243 | 220254 | 432 |
| 2 | 3318872 | 3318872 | 3318872 | 0 | 15360 | 0 | 63674 | 233 | 518.374 | 384.054 | 220248 | 432 |
| 3 | 3323998 | 3323998 | 3323998 | 0 | 11264 | 0 | 63674 | 233 | 521.749 | 430.941 | 220766 | 434 |
| 4 | 3320920 | 3320920 | 3320920 | 0 | 12288 | 0 | 63674 | 233 | 536.338 | 430.169 | 220760 | 434 |
| 5 | 3317854 | 3317854 | 3317854 | 0 | 12288 | 0 | 63674 | 233 | 542.857 | 429.040 | 220766 | 434 |

### a56 six-owner run

| Owner | Accepted B | Written B | Durable B | Pending B | Capture HWM B | Backpressure count | Subscriber HWM B | Receipts | Max healthy age ms | Max healthy gap ms | Indexed frontier B | Passages |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | 2903317 | 2903317 | 2903317 | 0 | 69632 | 2 | 63236 | 200 | 478.739 | 389.017 | 231012 | 454 |
| 1 | 3484944 | 3484944 | 3484944 | 0 | 11264 | 0 | 64771 | 246 | 463.723 | 418.969 | 231007 | 454 |
| 2 | 3483914 | 3483914 | 3483914 | 0 | 13312 | 0 | 64929 | 246 | 509.267 | 396.138 | 231001 | 454 |
| 3 | 3490064 | 3490064 | 3490064 | 0 | 12347 | 0 | 64929 | 246 | 501.712 | 396.743 | 231007 | 454 |
| 4 | 3483914 | 3483914 | 3483914 | 0 | 12288 | 0 | 64929 | 246 | 525.163 | 396.870 | 231001 | 454 |
| 5 | 3486992 | 3486992 | 3486992 | 0 | 12288 | 0 | 64929 | 245 | 531.540 | 395.054 | 231424 | 454 |

The maximum healthy accepted age at durable completion was 542.857 ms originally and 531.540 ms in the strengthened run. The maximum inter-completion gap within active healthy output was 430.941 ms originally and 418.969 ms in the strengthened run. These finite observed receipt distributions meet the healthy one-second operating target. They do not establish a maximum crash-loss window, physical fsync power-loss guarantee, or a one-second guarantee under storage failure.

The selective fault produced observed backpressure on owner 0. Owners 1–5 retained `storage_error: null` and each advanced its durable prefix by more than 65,536 bytes during the peer-fault/search/diff interval. They ended with distinct identities, independent prefixes and no pending raw bytes. No buffer or timeout was increased for this measurement. Observed high-water marks are samples of this workload, not replacement capacity constants or allocator maxima.

Indexing is explicitly partial. The copied quiescent stores showed about 220 KB indexed per session in the original run and about 231 KB per session in the strengthened run, versus millions of durable raw bytes. Every strengthened session had 454 committed passages, `caught_up: false`, and `fully_sealed: false`. Fair derivative progress does not imply that all retained output is immediately searchable. The original logger queried a nonexistent `partial` response field and recorded null; that is not a measured partial-index flag. The strengthened logger records the real `Browse` response (`request`, `rows`, `after`, `more`, `preparing`, `opened`, `error`) plus independently queried copied-SQLite frontier/ordinal/sealed/durable facts after confirmed shutdown. The actual Response schema has neither a `partial` nor an `indexing` field.

The original run completed 108 healthy and 24 fault searches, and the same counts of actual Working comparisons. The strengthened run completed 102 healthy and 24 fault searches/comparisons; all 126 recorded final search responses had no error, and 118 reported more result pages. Search results were bounded indexed-prefix results. The analyzer’s generic row-count distribution happens to retain `_us` key names; those values are counts of rows (maximum 32), not search latency. The latency table above uses the actual separate request-to-result microsecond samples.

## RSS, disk and cleanup

| Run | Owner peak KiB | Harness peak KiB | Child peak KiB each | Dashboard peak KiB | Transcript logical B | Final transcript allocated B | SQLite family B |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 335 | 15984 | 9120 | 3904–3936 | 9824 | 557 → 19355103 | 19369984 | 233472 → 5423640 |
| a56 | 17184 | 9920 | 3920–3936 | 9648 | 563 → 20333145 | 20344832 | 233472 → 5812920 |

The sampler ran approximately once per second and recorded actual sampler duration, `ps` RSS, and every flat state-file logical/allocated size. SQLite-family size includes the database and any WAL/SHM files at the sample. Process identities distinguish the harness, owner, six producers and each short-lived dashboard; all per-PID first/last/peak values are in `analysis.json`. The manifest’s background inventory uses `comm`, avoiding unrelated full argument capture. Detailed argv sampling is limited to the fresh owned processes. These short runs show observed growth and peaks, not sustained memory plateaus, native-agent RSS, or a storage-capacity guarantee.

Normal Shutdown was confirmed after final capture settled. The strengthened bundle contains a read-only fresh-owned-process exit audit: the observed harness/owner/children/dashboard PIDs were absent after completion. No stored PID conferred signal authority and no unrelated process was signalled. Both fixture workloads used temporary private state; the bundles retain the quiescent saved SQLite/transcripts and exact run boundaries.

## Claude Code 2.1.319 behavior supplement

This small supplement used a separate private root at `/Users/christopherbengtsson/.rat19-smoke319-a56eee5`, separate state/controllers, an immutable copied product and parser, and a proof-only PATH pointing to the actual default resolved Claude executable `/Users/christopherbengtsson/.local/share/claude/versions/2.1.319`. Its SHA-256 was `1f53587e92dee8e089fa349188a4cda64fa34ace6e7b0ca2ee278ddaa3cc2ebd`. No manual global configuration, credentials, permissions policy or native-history deletion occurred. Only the normal native trust choice for this newly owned checkout was approved; the preexisting native auto-mode setting was left untouched. The separate main 318/167 proof ran concurrently, so no timing/resource claim follows from this smoke.

The runner SHA-256 was `aafa4aeaf23a2b39b836eb75f93121f6cecb51b842af4e71cddc71ac563406ea`; parser SHA-256 was `def65d10dc1d08ae916528ff4b3b09474844b9bb6c58ee21b7a70d31d784cd6a`. Source, product, runner, parser and Claude hashes matched afterward; the source and checkout remained clean.

Demonstrated through real native input and parsed terminal captures:

- Initial prompt exact response SMOKE319-CEDAR-714.
- Actual AskUserQuestion choice Cedar/Amber; Amber selected and SMOKE319-CHOICE Amber response.
- Actual native working esc-to-interrupt footer followed by Ctrl-C; working indicator removed and native process retained.
- 160x40 and60x18 native viewport, Ctrl] focus escape, same-current-session attach.
- Detach/reconnect same original process and managed/native IDs.
- Native /exit with retained original native resume UUID.
- Retained actual search rows and committed readable passage opening.
- Explicit ResumeSelected original managed segment launches same original native UUID; actual marker recall with new prompt omitting remembered token.
- Explicit Stop then separate same-native-ID prepared resume and ordinary quit stops live native CLI and supervisor.

The original native process PID 15237 stayed identical across six launch/trust/question/answer/interruption/resize drives, including detach and reconnect. The first managed segment was `3f9399f9-3bd0-4ead-9fa9-668dafb88b04`. Explicit `ResumeSelected` used that original segment and launched managed segment `3502f4d9-7582-4946-8e57-6df6df7fd1b0` with the same native conversation UUID `9fd9decb-540d-4e50-a1ca-46019fbd98ae`, recorded in actual `--resume` argv. Its new recall prompt omitted the remembered token; the actual response was `SMOKE319-RECALL-882 SMOKE319-CEDAR-714`. A further prepared resume of the same original native conversation was used only to test ordinary quit against a live CLI, after separately proving Stop.

| Retained managed segment | Final lifecycle | Accepted = written = durable B |
| --- | --- | --- |
| 3f9399f9-3bd0-4ead-9fa9-668dafb88b04 | Exited | 22129 |
| 3502f4d9-7582-4946-8e57-6df6df7fd1b0 | Stopped | 7083 |
| 0c6458fb-f770-4006-ac75-b76ef4baac77 | Stopped | 2664 |

All three managed histories and the same original native UUID remain retained; no native history was deleted. After ordinary quit, the private supervisor endpoint was absent, and fresh owner PID 12682/native PID 64734 were absent from the read-only process check. Quit used the normal product action; no manual signals were used for native/owner cleanup. The separate failed dashboard attempts below killed only their current live dashboard Popen handles while the native session was already Exited.

All observed 2.1.319 semantic states remained **Unknown**, including during the genuine AskUserQuestion choice, native working UI and Ctrl-C. Lifecycle Running is process evidence, not semantic working proof. The native working footer `esc to interrupt` was captured before Ctrl-C; afterward that native working indicator disappeared and the input prompt returned. This demonstrates interaction without claiming a dependable 2.1.319 semantic monitor.

Two failed harness attempts are preserved: `08-retained-readable-history` wrongly expected the later marker on page 0, which actually contains retained trust/startup chronology; `09-history-search-results` captured genuine search hits but left the browser open, where literal `d` is consumed, so its default detach ending timed out. The corrected `10-search-open-committed-readable-passage` explicitly closed the browser and passed search-hit and committed-readable-passage assertions. These failures are not silently counted as successful driver runs. Darwin returned ENOTTY when `tcgetattr(slave)` was attempted after the controlling-PTY leader exited; terminal restoration is recorded unavailable, never true. Captures are parsed terminal cells, not a Terminal.app or physical-screen guarantee.

## Artifacts and remaining gates

| Bundle | SHA-256 |
| --- | --- |
| [Original 335 current-owner measurement](m2-19-measure-current-owner-evidence.tar.gz) | `b8ba17de64ceee41111f88ab26628906e6599122bfaf0388a312cae7397f9951` |
| [Strengthened a56 six-owner measurement](m2-19-measure-six-owners-evidence.tar.gz) | `3951401fa9a8744e5a3a8c36fae6adc64d7735fcad0a85ed7546aa4aaee0f172` |
| [319 behavior smoke](m2-19-claude319-evidence.tar.gz) | `351f2c3776802203ebc453f94bbcebdfb688a0a23559234caca9a35258f98cb5` |

The fixture bundles include raw observations, source/binary before/after stamps, exact logs, SQLite/transcript copies, analysis, and scope notes; the strengthened bundle includes its exact standalone analyzer and owned-process audit. The smoke bundle includes immutable tooling, plans, successful and failed raw/cell captures, public observations, resume argv/IDs, retained private state and checkout, boundary stamps and summary. The linked bundles are retained in this repository; the physical-gate state and executable are also retained at their private persistent roots.

The [main native workflow](m2-19-native.md) and [qualification record](m2-19-qualification.md) record the integrated workflow, installed editors, local recovery and visual review. Final integration review/checks remain separate. Actual sleep/wake and physical reboot are not established by these fixture or smoke runs. A physical reboot requires the user’s explicit authorization after the complete durable handoff is reviewable; no reboot was performed here and no injected boot fixture is being substituted for it. Ticket 19 stays unresolved and C6 remains unclaimed until its required physical recovery gate is demonstrated. No GitHub/reviewer/notification release gate is claimed.
