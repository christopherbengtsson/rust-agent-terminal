#!/bin/sh
# Run from the environment setup command; all installs belong to the current user.
set -eu

case "${1:-}" in
    ''|--with-claude) : ;; # Retain the old flag for saved environment commands.
    *) echo "Usage: $0 [--with-claude]" >&2; exit 2 ;;
esac

cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
for dependency in curl git cc make python3 node npm npx; do
    command -v "$dependency" >/dev/null 2>&1 || {
        echo "Missing prerequisite: $dependency (see docs/cloud-development.md)" >&2
        exit 1
    }
done

node -e 'const [major, minor] = process.versions.node.split(".").map(Number); if (major < 22 || (major === 22 && minor < 20)) { console.error("Skills setup requires Node.js 22.20 or later"); process.exit(1); }'

# rustup's default directory may not be on PATH in non-interactive setup shells.
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$HOME/.local/bin:$PATH"
if ! command -v rustup >/dev/null 2>&1; then
    installer=$(mktemp)
    trap 'rm -f "$installer"' EXIT HUP INT TERM
    curl --fail --show-error --silent --location https://sh.rustup.rs -o "$installer"
    sh "$installer" -y --profile minimal --default-toolchain none --no-modify-path
    rm -f "$installer"
    trap - EXIT HUP INT TERM
fi

# rust-toolchain.toml selects stable and the repository's required components.
cargo --version
rustc --version
cargo fetch --locked
cargo build --locked

# Install the AI Hero skills pack for both supported coding agents without prompts.
npx --yes skills@latest add mattpocock/skills --global --skill '*' --agent codex claude-code --yes

# Preserve platform-provided Codex installations and their authentication wiring.
if ! command -v codex >/dev/null 2>&1; then
    npm install --global --prefix "$HOME/.local" @openai/codex@0.167.0
fi
npm install --global --prefix "$HOME/.local" @anthropic-ai/claude-code@2.1.318
codex --version
claude --version
codex --help | grep -q -- '--no-daemon' || {
    echo 'Codex must support --no-daemon for this project' >&2
    exit 1
}

if [ -n "${AGENT_OPENAI_API_KEY:-}${OPENAI_API_KEY:-}" ]; then
    sh scripts/cloud-codex-auth.sh
else
    echo 'Codex API authentication pending: add AGENT_OPENAI_API_KEY in cloud settings, then run sh scripts/cloud-codex-auth.sh.'
fi

echo 'Setup complete. Add $HOME/.cargo/bin and $HOME/.local/bin to the environment PATH.'
echo 'Run sh scripts/cloud-check.sh to validate development support.'
echo 'Run sh scripts/cloud-agent-check.sh to check local authentication, then add --live to verify provider access.'
echo 'Before native qualification, build with cargo build --locked --features native-agent-qualification. Ordinary builds ignore the test switch.'
