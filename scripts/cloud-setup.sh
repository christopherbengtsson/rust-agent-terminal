#!/bin/sh
# Run from the environment setup command; all installs belong to the current user.
set -eu

case "${1:-}" in
    '') install_claude=false ;;
    --with-claude) install_claude=true ;;
    *) echo "Usage: $0 [--with-claude]" >&2; exit 2 ;;
esac

cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
for dependency in curl git cc make python3; do
    command -v "$dependency" >/dev/null 2>&1 || {
        echo "Missing prerequisite: $dependency (see docs/cloud-development.md)" >&2
        exit 1
    }
done

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

if "$install_claude"; then
    command -v npm >/dev/null 2>&1 || {
        echo "--with-claude requires Node.js and npm" >&2
        exit 1
    }
    npm install --global --prefix "$HOME/.local" @anthropic-ai/claude-code@2.1.318
    claude --version
fi

echo 'Setup complete. Add $HOME/.cargo/bin and $HOME/.local/bin to the environment PATH.'
echo 'Run sh scripts/cloud-check.sh to validate development support.'
