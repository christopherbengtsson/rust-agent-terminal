# Agent terminal

A Rust terminal workspace for coding agents. The product name is undecided;
`agent-terminal` is the current executable name. See the [vision](VISION.md),
[product requirements](PRODUCT.md), [architecture](docs/architecture.md), and
[implementation milestones](docs/milestones.md).

The dashboard discovers real Git worktrees, explicitly adopts a checkout, and
launches Claude Code (`c`), Codex CLI (`x`), or a scripted PTY fixture (`l`). Native
launch is experimental; see the recorded version-specific proofs below. Durable
readable history, supervisor reconnect, conservative launch reconciliation and
explicit native recovery are implemented. Native status has bounded per-version
evidence; GitHub workflows remain unavailable. The first release targets macOS;
fixture tests do not establish native-agent compatibility.

The [integrated Milestone 1 evidence](docs/proofs/18-milestone-evidence.md) records
exact native versions, measured capacity and limits. The discovered default
Codex foreground-subprocess Stop failure was fixed and real Stop/quit retests
passed for both recorded tools. Final source checks and resource revalidation
also passed for this scoped checkpoint; full release coordination and GitHub
workflows remain later work. The [final review corrections](docs/proofs/18-review-fixes.md)
record the newer interaction/identity/process-outcome checkpoint and its checks;
historical native and resource measurements retain their original boundaries.

## Run

Install stable Rust and Git, then run from an interactive terminal:

```sh
cargo run --locked
cargo run --locked -- /path/to/repository
```

With no argument, the dashboard inspects the current directory. You can point
it at any directory inside a repository or one of its linked worktrees.
Discovery reads Git metadata; it does not create worktrees or start agents.
Errors appear in the dashboard, where you can retry or quit.

Project and workspace identities survive restart, branch/name changes and missing
folders. Canonical Git common directories deduplicate project aliases; linked
checkouts retain distinct workspace identities. Re-adoption selects retained work.
Legacy tasks whose repository/path was never recorded appear under Unknown
project with checkout unknown; the dashboard does not infer a path from display
text. Workspace history remains readable when its checkout is unavailable.
Adding, selecting, discovering and re-adopting never start an agent. The active
session remains separate from browsing; stop it before adopting another checkout.
The workspace picker temporarily occupies the session pane, including in small
terminals; normal navigation retains Reviews above the worktree sidebar.


| Key | Action |
| --- | --- |
| `j` / Down | Select next worktree |
| `k` / Up | Select previous worktree |
| `r` | Refresh worktrees in the background |
| `f` | Inspect the selected retained workspace’s Working diff; in the worktree list, adopt an unknown checkout first |
| `w` | Open retained projects/workspaces; available in small terminals |
| `A` | Enter an existing local repository path and register it without adopting or starting coding |
| `U` | Edit optional project setup shell command; Enter saves, empty disables, Ctrl-U clears, Esc cancels |
| `T` / `Y` / `I` in creation receipt | Retry setup / Continue without setup (launch effect shown) / informed reconciliation or retry after uncertainty |
| `N` | Create an independent workspace for the selected registered project; Tab edits base/branch/folder/prompt/agent, Left/Right changes agent |
| Enter / Ctrl-S in creation | Create without starting / Create and start the chosen native agent |
| `O`, `[` / `]` in creation receipt | Open creation receipts / select the original operation by stable identity |
| `L` / `G` / `Z` in creation receipt | Explicit pinned local fallback after fetch failure / query or recover without starting / cancel pending creation |
| `j` / `k`, `[` / `]` in workspace picker | Select a workspace or jump between projects |
| Enter / `a` / `h` in workspace picker | Discover selected project's worktrees / explicitly adopt selected checkout / read its latest retained session |
| `w` / Esc in workspace picker | Return to the native session pane without changing execution |
| `a` | Explicitly adopt the selected usable checkout |
| `C` / `X` | Enter an optional first prompt, then Enter explicitly starts Claude / Codex; Esc cancels |
| `R` | Edit the selected workspace name; Enter saves, Ctrl-U clears, Esc cancels |
| `c` | Start a new Claude Code conversation in the adopted checkout (experimental) |
| `x` | Start a new Codex CLI conversation with `--no-daemon` (experimental) |
| `v` | Explicitly resume the latest managed native conversation using its saved native ID |
| `l` | Launch the scripted PTY fixture in the adopted checkout |
| Enter | Attach to the existing managed session |
| Ctrl-] | Return from attached focus to dashboard navigation |
| `s` | In dashboard focus, stop the managed native/fixture session and keep the dashboard open |
| `g` | Inspect history cleanup; `j`/`k` select, `n`/`p` page, Enter preview, `D` confirm selected deletion, Esc cancel/back |
| `y` | Retry history storage after repair; the sole writer retains pending bytes |
| `d` | In dashboard focus, detach and exit while preserving execution |
| `q` / Esc / Ctrl-C | In dashboard focus, request supervisor shutdown and quit after confirmed stop |

Working inspection always opens a fresh read-only snapshot of staged, unstaged
and new files. `j`/`k` selects changed files; arrows scroll content, Page Up/Down
moves eight lines, and Home returns to the start. `r` refreshes while preserving
the selected file/context where available; `f` or Esc returns. Workspace,
checkout, request and capture time identify the snapshot: it is not a live or
atomic view of files an agent can still change. Stop, quit and detach remain
available. Binary, conflict, submodule, missing/unsupported and oversized content
have explicit explanations. Inspection neither changes the index nor runs
repository-defined external diff, text conversion or fsmonitor programs.

Within inspection, `w` explicitly switches between Working and Whole workspace.
Whole workspace compares the common ancestor of the current HEAD and the selected
local base with current files, including earlier commits, staged/unstaged changes,
and new files. `b` selects a full local ref such as `refs/heads/main` or
`refs/remotes/origin/main`; Enter durably saves that base and compares, Esc cancels.
The selected base ref and its commit are retained separately from the current
branch. Ref movement uses the new local tip and labels the original selection;
branch, HEAD, actual base tip and common ancestor identify the snapshot. No fetch
runs during inspection. Missing bases, unrelated history and multiple common
ancestors require an explicit related base choice. Every new inspection still
opens Working.

Inside either comparison, `e` opens a fresh **full comparison inspection** in the
configured editor. The owner generates bounded content for every inventoried
file, identifies HEAD/base/common ancestor where applicable, and labels binary,
conflict, oversized or omitted content. The dashboard materializes a private,
read-only `.diff` outside the checkout. That artifact stays unchanged while open;
`e` again captures fresh content. The current selected file is positioned in the
artifact where supported.

`o` opens the selected **live source file** for editing; `E` opens the live
workspace directory from dashboard navigation or inspection. These files are
shared with the active agent: saving changes the workspace. The owner checks the
retained checkout identity and returns only an existing regular source file within
it. Deleted/missing files remain inspectable through `e`; source editing never
substitutes a fabricated filename. Vim positions source at the new-side hunk
nearest the visible diff line. This is a local-host source editing capability;
remote source editing is not implemented.

Vim is the default. Configure an executable and literal argument list with
`RAT_EDITOR_JSON`, for example:

```sh
RAT_EDITOR_JSON='{"executable":"/usr/bin/vim","arguments":["-u","NONE"],"position":"vim"}' cargo run --locked -- /path/to/repository
RAT_EDITOR_JSON='{"executable":"/usr/bin/pico","arguments":[],"position":"plus"}' cargo run --locked -- /path/to/repository
```

`position` is `vim` (line position and read-only inspection options), `plus`
(`+line`), or `none` (the default for custom configurations). Paths are absolute
argv operands; no shell parses executable, arguments, or paths. GUI editors must
have a wait argument configured so dashboard ownership returns after editing,
for example `arguments: ["--wait"]` for an editor supporting that option.
Terminal ownership is released while the editor runs and reacquired after success,
nonzero exit, or spawn failure. Selection, focus, history and diff context remain
in memory, the dashboard redraws at the actual terminal size, and agents continue
in the supervisor. Editor errors appear in the footer; the next key restores the
usual keyboard help.

In attached focus, UTF-8 characters, Ctrl-C, legacy control/Alt shortcuts,
arrows, navigation keys, F1–F12 and Shift-Tab reach the native/fixture PTY. The pane and
help show input focus; attached `q` and Esc are native input. Ctrl-] (the same
legacy byte as Ctrl-5) returns to dashboard focus without stopping the session;
Enter reattaches to the same child. Both recorded native versions exercised this
chord; other keyboard protocols remain outside the proof. Ordinary quit visibly
waits for the supervisor's confirmed
stop. Stop is forceful: a worker verifies live descendant identities across
process groups, freezes that owned scope to a stable closure, leaves TERM pending
while frozen and sends KILL after 500 ms. It confirms leader exit and absence of
executing owned members within two seconds. Native shutdown hooks and native
conversation buffers are not guaranteed to flush; use the native CLI's `/exit`
for graceful native shutdown. Managed captured history still has its separate
EOF/durable-flush boundary. Inventory, identity or bound failures remain visible
and retryable with `s`/`q`; they never count as confirmed cleanup.

Live ancestry is sampled every 100 ms, with at most 1024 retained identities per
session. Inventories cap at 8192 processes, 1 MiB and 500 ms; closure has a 500 ms
budget inside the existing two-second deadline. Unrelated processes outside that
ancestry are excluded. Deliberate double-fork/reparent escape between samples is
outside this proof; no kernel sandbox or elimination of the OS check/signal race
is claimed. Restored uncertain records never gain signal authority from saved
PIDs. See [the native child correction](docs/proofs/18-native-child-stop.md).
Native exit retains the latest screen and task/session attribution and returns
to dashboard focus. A successful native process exit is `Exited`; a nonzero exit
or signal is `Failed`, with its original reason retained in metadata and readable
history through later storage/control errors and reload. A failed intermediate
command while the native CLI continues does not make the session Failed.
Explicit Stop/quit of a running process remains `Stopped`. A subsequent explicit
launch creates a new session after execution cleanup and final history flush.
Neither native exit nor stop deletes working files. Detach restores the terminal
and closes only the dashboard connection. Unexpected dashboard loss also preserves
execution. Quit waits for durable history flush; a failed flush remains visible and retryable.

Bracketed paste is enabled on the outer terminal and forwarded with delimiters
when the child requests that mode; otherwise multiline paste normalizes line
breaks to Enter. A paste over 4084 UTF-8 bytes is visibly rejected for retry in
smaller pieces. The scripted child reports exact bytes, accepts `approve` then
`y`/`n`, handles Ctrl-C as an interrupt, and `identity` reports its PID.

Encoding uses legacy xterm sequences. Super/Hyper/Meta and unmappable control
characters report an error. Legacy terminals cannot distinguish every modifier
combination (for example Ctrl-Enter versus Enter), or Ctrl-] versus Ctrl-5.
Keyboard enhancement protocols, mouse forwarding and IME composition guarantees
remain unimplemented. Child cursor-key mode is respected by the supervisor at encoding time. Paste requires terminal bracketed-paste support; Crossterm buffers an entire
incoming paste before the application can reject its size. See the
[input proof](docs/proofs/03-native-input.md) for the tested bytes and boundaries.

The supervisor runs as another mode of the same executable, with a private local
socket under `~/Library/Application Support/rat/proof`. For isolated experiments:

```sh
cargo run --locked -- /path/to/repository --state-dir /tmp/rat-proof-example
```

One dashboard controls the supervisor at a time. A second dashboard reports the
existing connection and cannot launch, type into, or stop its session. After a
controller dies, reopen the dashboard to recover the same session and screen.
Symlink checkout aliases and paths inside a checkout adopt its canonical Git
worktree root. A new launch while running is refused; Enter attaches to the existing
child. The surviving supervisor retains launch-request outcomes: reconnect
recovers the original request and its success or failure, and retrying that exact
request does not execute again. An explicit `c`, `x` or `l` after an acknowledged outcome is
a new request. This recovery applies to dashboard loss; supervisor loss needs the
separate crash-reconciliation work. See the [reconnect proof](docs/proofs/07-dashboard-reconnect.md).

The state directory must be owned by the current user with mode 0700 and cannot
itself be a symlink. The socket is mode 0600. Concurrent starts use an OS ownership
lock; only its owner can remove a stale, refused socket. A live or uncertain
endpoint is never displaced. Keep the lock file in place, including after shutdown.
Use separate private `--state-dir` directories only for isolated experiments.

The durable-history and launch-recovery sections below describe current storage
and recovery behavior. Unobserved processes that escape live ancestry
remain outside the proof; interrupted native execution can require verified reboot
before replacement. Dashboard loss preserves the supervisor and session; reconnect and quit
normally to clean up. Do not remove a socket for a running supervisor. The bounded
parsed screen follows the session pane viewport (up to 200×80 cells), without
scrollback; it is not history. The PTY and parser resize together, including
while dashboard focus is active. Small terminals hide the sidebar and retain
a short session identity, focus/status and essential actions. Native cell colors,
styles, wide Unicode and cursor are rendered within the pane; child terminal
controls never run directly on the outer terminal. Mouse requests are parsed but
mouse input is not forwarded. See the [screen proof](docs/proofs/04-session-screen.md)
for geometry, captures and parser limitations.
See the [first-slice evidence](docs/proofs/01-scripted-pty.md) and
[ownership evidence](docs/proofs/02-session-ownership.md) and
[lifecycle evidence](docs/proofs/06-lifecycle.md) for tested scope.

## Development

```sh
cargo build --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

CI runs these checks on pull requests on macOS and Linux. Tests create isolated
temporary repositories and do not require agent accounts or GitHub access.

Claude Code launch uses `claude` from the supervisor's inherited PATH and the
existing native login. Install Claude Code and run `claude auth login` in your
usual terminal if missing or unauthenticated, then reopen the dashboard if PATH
changed. Authentication/setup prompts remain in the native CLI. A failed spawn
retains the adopted checkout and discovery; `c` is an explicit retry. A running
session refuses replacement regardless of the selected launch choice. Launch is
experimental: see [the native proof](docs/proofs/05-claude-native.md) for the exact
version and bounded scenarios. Native tool usage can consume your account's
allowance. No credential copying or product authentication store is used.

Native status is separate from process lifecycle. For the demonstrated canonical
Claude 2.1.318 installation, a passive invocation-local monitor can show working,
input, Bash approval, completed turns and acknowledged interruption. Hook events
must agree with current native UI and bounded freshness/generation checks. Other
versions/layouts and missing, stale or ambiguous evidence stay unknown; pre-output
thinking and some native footer variants also remain unknown. No user hooks,
permissions or persistent settings are replaced. See [the exact-version status
proof](docs/proofs/10-claude-status.md) for the tested scenarios and limits.

Codex CLI launch uses `codex --no-daemon` from the supervisor's inherited PATH,
keeping the managed interactive runtime outside Codex's shared daemon. It reuses
existing native authentication; run `codex login` in your usual terminal if
needed. Native authentication/setup prompts stay in the CLI. Failed spawn keeps
the adopted checkout; `x` explicitly retries. Stop the current session before
changing tools: launching the other tool starts a new conversation and does not
transfer context. See [the Codex native proof](docs/proofs/11-codex-native.md)
for the tested version and limitations. For Codex 0.167.0, passive invocation-local
hooks plus demonstrated native UI evidence can show working, input, approval,
completed turns and interruption. Codex may ask you to review/trust those exact
hook definitions; without valid fresh evidence the dashboard shows unknown.
Working evidence expires after five seconds; modal/completion/interruption
confidence after 30 seconds. Different versions/layouts and ambiguous completion
markers remain unknown. No hook approves commands or changes your policy.
See [the versioned status proof](docs/proofs/12-codex-status.md) for the native
scenarios and conservative limits.

Readable history is independent of a live child, native resume, and the checkout.
In dashboard focus press `h` to open/close the current session’s retained history.
Press `B` to browse conversations by stable session identity; `n`/`p` from a
UUID-selected history page return to this list. `[`/`]` select raw byte ranges,
and `j`/`k` scroll the current page.
The default readable view decodes UTF-8 across pages, removes terminal styles and
hidden control strings, and follows bounded line edits and horizontal cursor
movement. Replay runs in background batches; a cold distant page can show
Preparing readable history while capture and controls continue. Press `v` inside
history to toggle the escaped raw view. Outside history, `v` remains native Resume.

Press `/` for Find workspace (project/title/last observed branch/path), `S` for
Search history across all projects, or `B` for retained sessions. Enter submits
an edited query; arrow keys select results; Enter opens the selected workspace’s
conversations, exact session, or matching passage. Tab changes search mode,
`n` requests the next bounded result page, Ctrl-R refreshes, and Esc returns.
Reading does not select another coding runtime, launch an agent, or change native
Resume’s target. On a selected conversation row, `v` explicitly resumes that row’s
original tool and native conversation. The row shows workspace, managed segment,
and native identity before the action. Enter only reads history.

History search matches all case-insensitive words (Unicode letters/digits; punctuation
separates words), up to eight words/256 query bytes. Metadata Find matches a
case-insensitive substring. Results contain bounded snippets and stable passage
IDs/ranges; opening a hit retrieves its persisted readable passage directly,
including when controls span many raw pages. The index catches up incrementally
to committed output; results are labelled partial and Ctrl-R refreshes. Completed
lines are indexed; an unfinished tail becomes searchable after final capture is
sealed. Raw bytes remain available through the session’s history view.

`K` previews Stop, save and archive. Enter confirms; Escape dismisses the
panel while the operation continues, `z` cancels archive, and `j`/`k` scrolls
receipts/exclusions. Stop remains an independent session action. Archive cancels
pending native/setup intent and waits for confirmed owned cleanup and durable
history. An uncertain Stop blocks saving. Quit stops outstanding local work;
detach leaves the supervisor available for reconnect.

A dirty workspace gets an **Archive checkpoint** commit using your Git identity,
hooks and signing settings. A clean workspace keeps its existing HEAD. The
receipt says **saved locally, not pushed**, lists ignored exclusions, and shows
the saved commit and workspace recovery ref. Files and chats stay in place.
The checkpoint combines tracked and new nonignored working changes. Git's
original staged tree is pinned separately at the receipt's ref plus `-index`,
including staged-only content no longer present in working files. For example,
`git show '<receipt-ref>-index:path/to/file'` reads that saved staged version.
A successful save leaves the index matching the checkpoint; partial staging is
combined as part of the save. Git save failures preserve the original real index;
a successful Git save followed by a metadata failure already has the combined
index and retains the staged recovery tree. Either failure leaves the workspace
unarchived; partial Git outcomes are reported explicitly.

Enter retries the same archive request after a partial outcome or lost reply.
If files changed since its prepared snapshot, cancel that operation and press
`n` for a new snapshot, then Enter. Existing recovery refs remain retained.
Conflicts, submodules/nested repositories, sparse/split/special index state,
missing identity, rejecting hooks, external edits and uncertain checks block
complete-save success with an explanation. Abrupt supervisor loss during Git
may leave Git locks; the app never removes uncertain locks automatically.
Verify no Git process owns a lock before resolving it through normal Git repair.
Before confirming archive, `b` toggles **Delete local branch on archive** for the
exact displayed local branch. It starts on for product-created branches and off
for adopted branches. Immutable creation provenance remains separate from the
last observed branch; an external rename/switch causes conservative retention.
Cleanup starts only after the saved recovery ref and archive metadata are
confirmed. It uses safe ordinary detach, retains the folder/ignored files, and
reports cleanup separately from save/archive success. Main/default/base branches,
branches in another checkout, external advances, uncertain checks, custom
`core.hooksPath` and executable `reference-transaction` hooks retain the branch.
Automatic deletion is currently qualified only for Git 2.50 with the files ref
backend. Other versions/backends retain the branch with an explanation. Remote
branches are never deleted. Git's worktree-use checks are immediate snapshots;
concurrent external worktree registration is not universally locked out.

Return explicitly restores a branch from the saved point before restoring active
visibility. A reused old name gets a fresh alternative; no existing ref is
overwritten. Failed switch, missing folder or changed recovery remains visible
and keeps the workspace archived. Coding always stays stopped. Enter retries the
exact displayed archive revision; a stale Return cannot affect a later archive.
Cancellation before branch mutation does not rearm on a late receipt. Once Git's
branch result is awaiting its durable acknowledgement, cancellation is too late
and the panel says so. Folder removal remains a separate later action.

`M` opens a separate **Remove workspace folder** preview for the selected
workspace, including an archived workspace shown through `w`, then `z`. It
freezes the canonical folder, current HEAD, branch, recovery ref and actionable
blockers. Unsaved tracked, untracked **and ignored files** block removal; move or
save them first, then `n` requests a fresh preview. An active or uncertain coding
session, setup, pending native-resume lookup, unfinished capture or archive
mutation also blocks removal. Removal never implicitly stops that activity.

When the preview is eligible, `y` explicitly confirms that exact folder/HEAD.
If code has no confirmed remote backup, press Space to acknowledge **local only,
not pushed**, then `y`. No remotes is reported as local-only. Without an upstream, live push-destination
evidence must independently cover HEAD to report remote backup; otherwise the
local-only acknowledgement applies. A configured but unavailable/missing remote
or uncertain push evidence blocks removal. Live push-destination checks can require connectivity/authentication;
changed evidence requires a fresh preview. The worker pins the exact current
commit at the displayed `refs/rat/removal/<workspace>/<request>` ref, including
never-archived/detached work, and saves durable intent before ordinary Git removal.
Main checkouts, locked/prunable worktrees and unsafe identities are excluded.
Git refusal is preserved; no force removal or recursive saved-path deletion is
used. Special/sparse/submodule indexes, shallow/partial clones and external Git
object stores are conservatively excluded when complete saved coverage is unknown.

Saved commits, old archive refs, all retained branches, workspace/native identities
and session history stay. For example, `git show '<removal-ref>:path/to/file'`
reads saved code through the project’s remaining checkout after folder removal.
Readable history/search remains usable without the removed checkout; in-place
native resume is unavailable. This action neither archives the workspace nor
deletes chats. `j`/`k` scroll the preview/receipt; Escape dismisses it, `d` detaches,
and `q` cancels pending mutation and quits after owned work settles. A mutation
already completed by Git cannot be rolled back by cancellation.

On a lost result or supervisor restart, reopen `M` and press `g` for **read-only
reconciliation** of the same request. Startup and reconciliation never replay
folder deletion. A safe retained folder requires a fresh `n` preview and explicit
confirmation; a replacement path or uncertain identity remains blocked for
inspection. Storage errors stop further mutation; `r` in the removal panel retries storage,
then `g` reconciles the receipt. Recovery refs have no
automatic expiration. Checks protect observed state; external tools can still
write after the final check, so avoid editing a workspace while removing it.

Archived metadata and confirmed missing/inaccessible folders are labelled without
requiring the checkout for reading. `K` opens Stop, save and archive for the
selected workspace; `w`, then `z`, includes archived workspaces and `K` opens
Restore branch and return without starting coding.
Branches are labelled **last observed** from Git discovery, never inferred from
the original creation branch. Refresh discovery to observe ordinary Git branch
changes. Legacy facts that were not recorded remain unknown. Explicit transcript
deletion invalidates prior results/passages and prevents their return after restart.
See the [search proof](docs/proofs/m2-11-retained-search.md) for bounds and evidence.

This is captured terminal output, not reconstructed chat messages. Full-screen
redraws can repeat text, leave fragments or spacing artifacts, and the last line
is provisional until later output finishes it. Exact raw bytes remain retained
independently; only committed bytes are shown. Passage locations use the stable
session UUID, projection version and raw byte ranges rather than screen rows.
Input keys/paste are not separately recorded, so hidden password input is not
copied into history. Output itself can contain sensitive text. See the
[readable-history proof](docs/proofs/m2-08-readable-history.md) for current native
examples, bounds and limitations.

The supervisor stores SQLite metadata and 64 KiB transcript chunks under
`~/Library/Application Support/rat/proof` (or the explicit isolated `--state-dir`).
Metadata is marked unconfirmed until its commit succeeds. Ordinary quit waits
for process cleanup, PTY EOF, transcript file synchronization and the metadata
commit. A failed write keeps quit visibly incomplete; repair storage and retry
`q`, or press `y` to retry capture without requesting quit. Long storage operations
and producer backpressure are visible; navigation and Stop remain available.
Stopping during a storage stall retains accepted bytes for repair. A finite
Stop-only raw tail (at most 256 blocks / 1MiB) allows macOS terminal exit to drain
without waiting for storage repair. If that reserve fills, Stop can remain
unconfirmed and retryable. Termination can lose
unread output in the native PTY/kernel during process termination; the dashboard
keeps that possible-loss notice. Another launch waits for the old capture tail
and final flush. There is no automatic history deletion. Supervisor loss restores interrupted
records without starting coding; unresolved execution blocks replacement launch.
An abrupt failure can lose output beyond the committed prefix. See
[the history proof](docs/proofs/08-durable-history.md) for tested boundaries and
preliminary healthy-storage observations.

### Explicit history cleanup

History remains until you explicitly delete a selected segment. The storage
worker samples available space on the state directory's filesystem once per
second and warns below **1 GiB (1,073,741,824 bytes)**; equality does not warn.
An unreadable capacity sample is reported as unavailable, never zero or healthy.
Press `g` in dashboard focus for an eight-session page with task/session identity,
age, allocated transcript bytes and eligibility. Oldest known stopped sessions
are suggested first; `n`/`p` page, `j`/`k` select, Enter previews the effect,
`D` confirms that selected UUID, and Esc cancels/back. Attached native input keeps
its own shortcuts. Age, exit, archival and pressure never authorize deletion.

Only stopped, exited or failed execution segments with a recorded accepted-PTY
EOF, successful final
flush and durable metadata, and no active/uncertain capture are eligible. Process
Stopped alone is insufficient. Legacy session ages remain explicitly unknown;
legacy stopped records without this final-capture proof stay unavailable rather
than inventing an age or safe state. Interrupted execution remains unavailable.

Deletion removes only the selected managed transcript chunks. Checkout files,
task working changes, native tools' conversation records, session/native IDs and
launch-request outcomes remain. A committed deletion intent hides that segment
from readable history before bounded unlinking begins. Failed/partial deletion
stays visibly incomplete; `g`, select, Enter, `D` explicitly retries it after
repair. Restart resumes only previously committed explicit deletion intents.
The directory is synchronized before the durable completed marker; completed
cleanup refreshes history navigation and re-samples available capacity.

Allocated bytes cover transcript files, including uncommitted tails; they do not
attribute shared SQLite pages or promise the same increase in filesystem free
space. Cleanup can leave capacity low or a capture fault unresolved. `y` retries
capture/final storage after repair; `s` stops execution. Only a real successful
write/sync/commit acknowledges capture recovery. See
[the cleanup proof](docs/proofs/15-explicit-history-cleanup.md).

### Recovering an interrupted launch

After supervisor loss, the dashboard restores **Interrupted** execution and
retained history without starting coding. Press `u` to reconcile, `h` for history,
or `d` to detach. A successful reconciliation retains the interrupted record;
choose `v` explicitly for native resume, or `c`, `x` or `l` for a new conversation.

A verified surviving process blocks replacement; recovery does not kill a
restored PID. If native execution was authorized and its leader has disappeared,
same-boot checks cannot rule out escaped descendants: a verified reboot may be
required before `u` can release the reservation. Missing process evidence stays
blocked. `s` and `q` do not falsely confirm that uncertain unmanaged execution
stopped. Launch requests and their known results survive supervisor restart;
retrying the same request does not start another agent. See the
[launch recovery proof](docs/proofs/09-launch-reconciliation.md).


### Explicit native conversation recovery

In dashboard focus, `v` resumes the latest managed native conversation. To choose
an older one, press `B`, select its conversation row, and press `v`. Both actions
resolve the saved segment through the storage worker and use its original tool
and native conversation ID: Claude `--resume <id>` or Codex `resume <id>
--no-daemon`. It supplies no coding prompt. Native folder/hook trust prompts stay
in the native pane. `c` and `x` explicitly start new conversations; unavailable
resume explains the reason and never silently starts a replacement. Live sessions
must stop first, and interrupted execution must pass `u` reconciliation before
resume or a new launch. Opening the dashboard never starts coding.

History (`h`) remains available when no native ID was captured or the checkout
is missing. A missing checkout prevents continuing in place; restore its path
before resuming. Native records remain owned by the original tool, which can
reject an unavailable ID through its captured native UI. A running process does
not prove native recovery succeeded. Readable history and native resume metadata
remain independent, including after explicit transcript deletion. Selected recovery
uses a new managed segment in the retained workspace and leaves Git’s current
HEAD, index and working files in place. See [selected recovery evidence](docs/proofs/m2-12-selected-native-resume.md)
and the earlier [latest-conversation proof](docs/proofs/16-native-recovery.md).
No physical reboot is claimed.

## Development measurement proof

The [paced measurement record](docs/proofs/17-session-measurements.md) includes
exact source/binary revisions, standalone raw artifacts, command logs, measured
limits and preserved failures. It uses isolated real Git repositories,
dashboards/supervisors and a scripted PTY; it does not establish native CLI
compatibility or the full release workload. Debug builds enable the explicit
storage-fault markers.

```sh
RAT_MEASURE17_DIR=/private/tmp/unique-main cargo test --locked --test ownership paced_session_measurement17 -- --exact --ignored --nocapture
python3 docs/proofs/17-analyze.py /private/tmp/unique-main
RAT_FAULT17_DIR=/private/tmp/unique-fault cargo test --locked --test ownership full_storage_input_receipt_measurement17 -- --exact --ignored --nocapture
python3 docs/proofs/17-analyze.py /private/tmp/unique-fault
```

Use a new output directory each time. `RAT_MEASURE17_QUICK=1` selects a short
preflight; it cannot replace the sustained resource/latency proof. Terminal-parsed
visible timings are not physical monitor timestamps. Full storage can block
native ACK output until repair even when input receipt and dashboard navigation
remain responsive.

Workspace labels are local metadata. A new adoption receives a stable generated
`Workspace <id>` fallback. Its first supplied prompt can replace that fallback
with a readable local label (collapsed whitespace, at most 72 characters).
Manual names and labels from an earlier prompt survive later launches. Renaming
never moves the folder, changes the branch or changes a native conversation ID.
The dashboard does not infer native titles from terminal output or submit a
hidden naming turn. Names saved by an older version are retained.

`C` / `X` opens an optional first-prompt editor; Enter is the explicit launch
step. Native interactive invocation receives that prompt as one positional
argument after `--`, alongside the normal passive status hooks. `c` / `x`
continues to start without a supplied prompt. Resume never supplies an initial
prompt. Startup, restoration, cancellation and reconciliation do not execute a
prompt. Independent workspace creation uses the same native launch boundary.

`N` creates an independent workspace in the selected registered project. The form
shows its base, suggested branch and stable folder, chosen agent and optional
first prompt; it requires no title. Choose a local branch explicitly when there
is no unambiguous remote default. With remotes configured, use an explicit remote
base (for example `origin/main`) or a local branch with a configured upstream.
Enter means Create without starting; Ctrl-S
means Create and start. Generated folders live under the supervisor state
folder's `workspaces/<project>/<operation>` directory, outside the original
checkout. Branch, folder and base overrides never overwrite existing work.

Creation normally fetches the selected remote branch and saves the exact ref and
commit before adding the checkout. A failed fetch stops at a visible local-code
choice. Its receipt shows the exact locally available commit; `L` explicitly
chooses that pinned version, including the displayed launch choice. Later ref
movement cannot change it. `G` queries or reconciles the original operation
without starting an agent. `Z` cancels pending creation; completed files/branches
are retained. Restore only reads saved receipts. A failed native launch retains
the created workspace, and a later explicit Start creates a new conversation.

Creation receipts retain separate checkout and native-launch results. `O` opens
receipts and `[`/`]` chooses an older operation. Late completion and background
launch do not change your current workspace selection. Native folder-trust,
hook-review and account prompts remain with the chosen CLI. See the
[creation evidence](docs/proofs/m2-09-workspace-creation.md).

`U` configures an optional project setup command, executed explicitly through
`/bin/sh -c` in each newly created workspace. Empty configuration runs no setup.
The exact command is frozen for each creation/attempt; later edits affect future
workspaces. Setup runs before the requested native launch, including when creating
without starting coding. `O` shows separate setup and native outcomes, with a
bounded combined stdout/stderr tail and discarded-byte count. Setup output is
local setup progress, not native conversation history. Live progress can be lost
until a phase receipt commits; the saved tail is not a complete transcript.

After failure, `T` retries the frozen setup command and `Y` explicitly continues
without setup. The receipt says whether that action will start the agent under
still-armed Create and start permission, or keep coding stopped. Bare Start is
refused until setup's failure choice is settled. `s`, `Z` and ordinary quit cancel
owned setup and disarm its pending coding launch; unrelated sessions keep running.
Restore never reruns setup or rearms launch. A lost result remains uncertain:
`I` performs read-only survivor reconciliation before an informed retry choice
that may repeat external effects. Saved PIDs are never used as signal authority;
some released executions require verified reboot to exclude escaped survivors.
A known terminal cleanup is recorded independently of command-outcome certainty.

One setup executes at a time; other native sessions remain usable. The current
bound retains 32 setup attempts per state directory and refuses additional
configured setup at that limit. Commands are limited to 4096 bytes and the output
tail to 8192 raw bytes; older output is explicitly discarded. A storage failure
retains the owned outcome for `y` retry after repair, without rerunning the command.
See [setup evidence and limits](docs/proofs/m2-10-project-setup.md).
