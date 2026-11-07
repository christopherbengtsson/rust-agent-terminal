#!/bin/sh
set -eu
cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$HOME/.local/bin:$PATH"

case "${1:-}" in
    '') full=false ;;
    --full) full=true ;;
    *) echo "Usage: $0 [--full]" >&2; exit 2 ;;
esac

cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
if "$full"; then
    python3 scripts/run-with-reaper.py cargo test --locked --all-targets -- --test-threads=1
else
    python3 scripts/run-with-reaper.py cargo test --locked --bin agent-terminal \
        --test editor --test scripted_session --test session_screen -- --test-threads=1
fi
python3 -m unittest discover -s scripts -p 'test_*.py'
