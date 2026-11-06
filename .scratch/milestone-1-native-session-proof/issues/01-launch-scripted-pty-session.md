# 01: Launch and attach to a scripted PTY session

Status: ready-for-agent
Blocked by: None

**What to build:** Explicitly adopt an existing checkout, launch a scripted native-style child through the supervisor, and interact with its emulated terminal in an image-guided dashboard. This is the first thin PTY/supervisor prototype, with no native-tool support claim.

- [ ] Opening or refreshing a real temporary Git repository discovers worktrees without adopting one, mutating repository files, or launching a child. Explicit adoption identifies the selected branch and checkout path; unusable, bare, or prunable checkouts receive an actionable explanation.
- [ ] An explicit launch starts one scripted PTY child in the adopted checkout, assigns stable task/session and launch-request identities, and renders its prompt and response in an attachable session pane. Arguments and paths are passed directly without shell interpolation; preserve non-UTF-8 paths internally.
- [ ] The supervisor runs as another mode of the same executable and owns the child and PTY over a user-private local socket. Only the dashboard owns the real terminal; child escape sequences are parsed into a bounded screen rather than printed over Ratatui.
- [ ] Use the reference image composition from this first usable slice: reviews above the project/worktree/session tree on the left, prominent session pane on the right, status labels and working keyboard help. Reviews are explicitly unavailable, with no nonfunctional controls or replacement message composer.
- [ ] Keep blocking process, Git, filesystem and socket work outside the UI loop with bounded event processing. Declare initial screen and queue caps, and clean up the prototype child after the demonstration.
- [ ] The primary integration test drives the real dashboard/supervisor and scripted child in temporary Git repositories with isolated state/socket locations. Assert child cwd, identity, observable input/output and read-only discovery; it must never connect to a developer’s real supervisor.
- [ ] Record prototype PTY/parser choices, a macOS smoke result and limitations. Keep one Rust package on the stable toolchain; update user instructions to distinguish fixture functionality from native compatibility.
