#!/usr/bin/env bash
# Remove only the user-facing binary; worker logs, registry, and credentials stay intact.
set -euo pipefail

INSTALL_DIR="${SAVANT_COLOSSEUM_BIN_DIR:-$HOME/.local/bin}"
INSTALL_PATH="$INSTALL_DIR/savant-colosseum"
if [[ -e "$INSTALL_PATH" ]]; then
  rm "$INSTALL_PATH"
  echo "Removed $INSTALL_PATH"
else
  echo "savant-colosseum is not installed at $INSTALL_PATH"
fi
echo "Retained worker records and JSONL logs at ${SAVANT_EXECUTIONER_HOME:-$HOME/.savant/colosseum}/workers"
echo "To remove retained local data manually: rm -rf ${SAVANT_EXECUTIONER_HOME:-$HOME/.savant/colosseum}"
