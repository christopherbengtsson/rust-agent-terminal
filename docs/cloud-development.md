# Cloud development for the macOS app

The app targets macOS. Linux cloud environments provide an editing, build and
portable-test workspace; they do not establish Linux product support or replace
the macOS native-agent, editor, performance and recovery checks in [testing.md](testing.md).

## Setup

Provide Git, curl, CA certificates, a C compiler/linker, Make, Python 3 and
Node.js 22.20 or later with npm/npx.
On Debian/Ubuntu these are `git curl ca-certificates build-essential python3`.
SQLite is bundled; no database service is needed. From the repository root:

```sh
sh scripts/cloud-setup.sh
```

The script installs user-owned rustup if missing, uses `rust-toolchain.toml`,
fetches locked dependencies and builds the app. It preserves the existing Git
installation. Git 2.50.x is a separate branch-deletion qualification requirement,
not a prerequisite for ordinary cloud editing and builds.

Setup also installs all skills from [Matt Pocock's AI Hero skills](https://www.aihero.dev/skills)
globally for Codex and Claude Code in the cloud user's home directory:

```sh
npx --yes skills@latest add mattpocock/skills --global --skill '*' --agent codex claude-code --yes
```

This runs without interactive prompts on each setup, using the latest skills CLI
and skills from that repository. It does not copy skills from your own computer.
Agents discover the installed skills when starting a new session.

Setup always installs Claude Code 2.1.318 and installs Codex 0.167.0 if Codex
is missing. An existing platform-provided Codex installation is preserved and
checked for `--no-daemon` support. The old `--with-claude` flag remains accepted.

Add `$HOME/.cargo/bin` and `$HOME/.local/bin` to the environment PATH, preserving
the existing PATH, proxy variables and CA trust. Save the setup command in the
cloud environment configuration for future instances. Machine-local installs
do not update that saved configuration.

## Development checks

```sh
sh scripts/cloud-check.sh
```

This portable baseline runs formatting, Clippy, build, bin unit tests, the editor,
scripted-session and session-screen fixture targets, and Python helper tests.
It serializes tests and uses a Linux-only child reaper so crash fixtures do not
accumulate orphaned zombies under container init. macOS CI is unchanged.
On SIGINT/SIGTERM, or when descendants outlive the command by five seconds,
the runner settles its owned children with TERM and escalates to KILL after
500 ms. It keeps reaping until every child is gone; unconfirmed cleanup keeps
the runner alive. Unrelated processes are excluded.

The repository's authoritative ordinary check remains `cargo test --locked
--all-targets`. To attempt it through the cloud runner:

```sh
sh scripts/cloud-check.sh --full
```

The ownership target contains ordinary fixture and business-behavior tests as
well as platform-sensitive scenarios; it is not all native macOS qualification.
The unmodified full target currently has Linux failures involving supervisor
reconnect frames, system-Git qualification, process-inventory deadlines and
streaming timing/backpressure assumptions. The cloud baseline does not run that
target and must not be reported as an all-target pass. Those failures are retained
for separate investigation; production transport and existing proof assertions
are unchanged by this setup. Ignored and native supplemental checks keep their
existing prerequisites in [testing.md](testing.md).

## Codex and Claude

Native-agent testing needs the real CLI, a PTY, provider access and authentication.
Installation does not require credentials. When `AGENT_OPENAI_API_KEY` (or
`OPENAI_API_KEY` outside this managed cloud) is available, setup initializes
Codex API authentication automatically. Otherwise, run
`sh scripts/cloud-codex-auth.sh` after the runtime secret becomes available.
Check both agents separately from the portable fixture tests:

```sh
sh scripts/cloud-agent-check.sh
sh scripts/cloud-agent-check.sh --live
```

The first command checks CLI versions and local authentication metadata. The
second also makes a small, billable inference request to each provider from an
empty temporary directory. The shared `scripts/agent-test-profile.json` owns the
current model, reasoning, and smoke-budget settings. Native qualification uses
explicit launch flags from the same profile; setup does not rewrite account
defaults or start billable agent conversations. Read
[agent cost controls](agents/cost-controls.md) before
any live test.

Native qualification additionally requires an explicit
`cargo build --locked --features native-agent-qualification`. Setup's ordinary
build leaves this feature disabled, so production launches ignore the test switch.
A cached login status can pass while live access
fails; only the live check establishes that a request succeeds now. Neither
command qualifies the app's PTY, resume, or native macOS behavior.

For Claude, use an Anthropic Console API key with API usage billing:

1. Create an API key in the intended organization at `https://platform.claude.com/`.
2. Add it as a secret named `ANTHROPIC_API_KEY` in the saved cloud environment.
   Make it available to runtime processes, including the supervisor and its
   Claude children, rather than only the setup command.
3. Save/publish the configuration and start an instance using that version.
4. Verify authentication in that instance:

```sh
claude auth status
```

New instances using that configuration receive the secret; new Claude
conversations inherit it from their runtime environment. No browser login is
needed for API-key authentication. Usage is billed to the key's Console
organization. Setup does not persist Claude's API key; Claude reads it from the
runtime environment. Codex API login separately writes its CLI authentication state.
Keep its value out of chat, repository files and setup commands.

Interactive `claude auth login --console` remains an alternative for a single
machine. That machine's saved login is not automatically copied to fresh cloud
instances.

For Codex, choose one supported authentication method:

- Use an existing platform-provided login, or run `codex login --device-auth`
  for ChatGPT account authentication. Account/plan eligibility applies; local
  login files are not automatically copied into fresh cloud instances.
- For repeatable API authentication in this managed cloud, add
  `AGENT_OPENAI_API_KEY` as a runtime secret scoped to `api.openai.com`, and
  initialize the CLI with `sh scripts/cloud-codex-auth.sh`. Cloud settings reserve
  the `OPENAI_` prefix; the helper accepts the conventional `OPENAI_API_KEY` in
  environments where that name is supported.
  This writes the CLI's user-owned authentication state; do not commit it.
  OpenAI API usage is billed separately from a ChatGPT subscription.

Keep keys in environment secrets, never chat or repository files. Preserve
platform-managed authentication unless intentionally switching to API billing.
Use `codex login status` followed by the live check above to verify the chosen
method. A stale account login needs to be renewed, not just reinstalled.

Allow Rust/package downloads (`sh.rustup.rs`, `static.rust-lang.org`,
`index.crates.io`, `static.crates.io`), GitHub, and `registry.npmjs.org` for the
CLI and skills installs. Claude API-key inference needs `api.anthropic.com`;
`platform.claude.com` is used for Console key management or interactive login.
Codex API-key access needs `api.openai.com`. Codex ChatGPT access uses `chatgpt.com` and can require
`auth.openai.com`. Enable additional hosts required by actual login redirects.
Saved network changes require an instance using that updated configuration.

The recorded semantic-status versions are Codex 0.167.0 and Claude 2.1.318.
Installing a CLI does not qualify its Linux runtime, status or resume behavior.
Claude's npm executable also lacks the canonical native-installation basename
checked by the status monitor, so status may remain Unknown. Leave those product
compatibility checks intact and perform release qualification on macOS.
