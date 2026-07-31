#!/usr/bin/env bash
set -euo pipefail

# Override any of these from the shell when running a different Savant setup.
export SAVANT_SERVER_URL="${SAVANT_SERVER_URL:-http://127.0.0.1:8090}"
export SAVANT_WORKSPACE_ID="${SAVANT_WORKSPACE_ID:-2539163563543949210}"
export SAVANT_EXECUTIONER_HOME="${SAVANT_EXECUTIONER_HOME:-/Users/home/.savant/colosseum}"

if [[ -z "${SAVANT_API_KEY:-}" && -z "${SAVANT_API_KEY_FILE:-}" ]]; then
  echo "SAVANT_API_KEY or SAVANT_API_KEY_FILE is required." >&2
  echo "Run: SAVANT_API_KEY='your-key' colosseum-runner" >&2
  exit 2
fi

if [[ "$#" -eq 0 ]]; then
  set -- worker --poll-seconds 15
fi

exec /Users/home/.local/bin/savant-executioner "$@"
