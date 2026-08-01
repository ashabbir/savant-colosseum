#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${GH_TOKEN:-}" ]]; then
  echo "GH_TOKEN is required for GitHub Releases publishing." >&2
  exit 1
fi

if [[ -z "${APPLE_ID:-}" || -z "${APPLE_APP_SPECIFIC_PASSWORD:-}" ]]; then
  echo "Warning: APPLE_ID / APPLE_APP_SPECIFIC_PASSWORD are not set." >&2
  echo "The app will build and upload successfully, but unsigned or unverified users may see a Gatekeeper warning on macOS until the app is signed." >&2
fi

export GH_TOKEN

npm run build -- --publish always
