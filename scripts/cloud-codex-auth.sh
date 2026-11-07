#!/bin/sh
# Never enable shell tracing here: the key must remain on stdin, not in logs.
set -eu
export PATH="$HOME/.local/bin:$PATH"
if [ -n "${AGENT_OPENAI_API_KEY:-}" ]; then
    printenv AGENT_OPENAI_API_KEY | codex login --with-api-key
elif [ -n "${OPENAI_API_KEY:-}" ]; then
    printenv OPENAI_API_KEY | codex login --with-api-key
else
    echo 'Add AGENT_OPENAI_API_KEY as a cloud secret scoped to api.openai.com before initializing Codex API authentication.' >&2
    exit 1
fi
