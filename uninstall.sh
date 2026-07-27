#!/usr/bin/env bash
# Uninstall savant-colosseum launchd agent and optionally remove data.
set -euo pipefail

HOME_DIR="$HOME"
PLIST_NAME="com.savant.colosseum"
PLIST_DEST="$HOME_DIR/Library/LaunchAgents/$PLIST_NAME.plist"
INSTALL_BIN="$HOME_DIR/.local/bin/savant-executioner"
UID_VALUE="$(id -u)"
LAUNCHD_DOMAIN="gui/${UID_VALUE}"

echo "→ Stopping savant-colosseum..."
launchctl bootout "$LAUNCHD_DOMAIN" "$PLIST_DEST" 2>/dev/null || true

if [[ -f "$PLIST_DEST" ]]; then
  rm "$PLIST_DEST"
  echo "→ Removed plist: $PLIST_DEST"
fi

if [[ -f "$INSTALL_BIN" ]]; then
  rm "$INSTALL_BIN"
  echo "→ Removed binary: $INSTALL_BIN"
fi

echo ""
echo "✓ savant-colosseum uninstalled"
echo ""
echo "  Data and logs are preserved at: $HOME_DIR/.savant/colosseum/"
echo "  To remove all data: rm -rf $HOME_DIR/.savant/colosseum/"
echo "  To remove service log: rm $HOME_DIR/.savant/colosseum.log"
