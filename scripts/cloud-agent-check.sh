#!/bin/sh
# Separate credential metadata from real, billable provider checks.
set -eu
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$HOME/.local/bin:$PATH"
case "${1:-}" in
    '') live=false ;;
    --live) live=true ;;
    *) echo "Usage: $0 [--live]" >&2; exit 2 ;;
esac

failed=0
for agent in codex claude; do
    if ! command -v "$agent" >/dev/null 2>&1; then
        echo "Missing CLI: $agent" >&2
        failed=1
        continue
    fi
    "$agent" --version
    if [ "$agent" = codex ]; then
        if ! codex login status; then failed=1; fi
    else
        if ! claude auth status; then failed=1; fi
    fi
done

if "$live"; then
    if ! python3 "$script_dir/agent_test_profile.py" smoke; then failed=1; fi
else
    echo 'Local authentication status does not prove provider access. Use --live for a billable smoke test.'
fi
exit "$failed"
