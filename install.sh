#!/usr/bin/env bash
# Install savant-colosseum (Executioner) as a launchd agent (macOS).
# Builds the release binary, installs the plist, and starts the service.
# Starts automatically on login, restarts on crash.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOME_DIR="$HOME"
PLIST_NAME="com.savant.colosseum"
PLIST_DEST="$HOME_DIR/Library/LaunchAgents/$PLIST_NAME.plist"
BINARY_NAME="savant-executioner"
INSTALL_BIN="$HOME_DIR/.local/bin/$BINARY_NAME"

# Configurable via env vars.
SERVER_URL="${SAVANT_SERVER_URL:-http://127.0.0.1:8090}"
API_KEY="${SAVANT_API_KEY:-}"
POLL_SECONDS="${COLOSSEUM_POLL_SECONDS:-15}"

# ---------------------------------------------------------------------------
# Pre-flight checks
# ---------------------------------------------------------------------------
if ! command -v cargo &>/dev/null; then
  echo "ERROR: cargo not found in PATH. Install the Rust toolchain first."
  exit 1
fi

if [[ -z "$API_KEY" ]]; then
  echo "ERROR: SAVANT_API_KEY is required."
  echo "  Usage: SAVANT_API_KEY='sk-...' bash install.sh"
  exit 1
fi

# ---------------------------------------------------------------------------
# Build the release binary
# ---------------------------------------------------------------------------
echo "→ Building release binary..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

# ---------------------------------------------------------------------------
# Install binary
# ---------------------------------------------------------------------------
mkdir -p "$(dirname "$INSTALL_BIN")"
cp "$SCRIPT_DIR/target/release/$BINARY_NAME" "$INSTALL_BIN"
chmod +x "$INSTALL_BIN"
echo "→ Installed binary to $INSTALL_BIN"

# ---------------------------------------------------------------------------
# Create data directories
# ---------------------------------------------------------------------------
mkdir -p "$HOME_DIR/.savant/colosseum/worktrees"
mkdir -p "$HOME_DIR/.savant/colosseum/logs"
mkdir -p "$HOME_DIR/Library/LaunchAgents"

# ---------------------------------------------------------------------------
# Render plist from template
# ---------------------------------------------------------------------------
sed \
  -e "s|BINARY_PATH|$INSTALL_BIN|g" \
  -e "s|HOME_DIR|$HOME_DIR|g" \
  -e "s|SAVANT_SERVER_URL_VALUE|$SERVER_URL|g" \
  -e "s|SAVANT_API_KEY_VALUE|$API_KEY|g" \
  -e "s|POLL_SECONDS_VALUE|$POLL_SECONDS|g" \
  -e "s|PATH_VALUE|$PATH|g" \
  "$SCRIPT_DIR/$PLIST_NAME.plist.template" > "$PLIST_DEST"

# ---------------------------------------------------------------------------
# Reload as a LaunchAgent for the current GUI user
# ---------------------------------------------------------------------------
UID_VALUE="$(id -u)"
LAUNCHD_DOMAIN="gui/${UID_VALUE}"
launchctl bootout "$LAUNCHD_DOMAIN" "$PLIST_DEST" 2>/dev/null || true
launchctl bootstrap "$LAUNCHD_DOMAIN" "$PLIST_DEST"
launchctl enable "$LAUNCHD_DOMAIN/$PLIST_NAME"
launchctl kickstart -k "$LAUNCHD_DOMAIN/$PLIST_NAME"

# ---------------------------------------------------------------------------
# Wait briefly to confirm the process started
# ---------------------------------------------------------------------------
sleep 2
if launchctl print "$LAUNCHD_DOMAIN/$PLIST_NAME" 2>/dev/null | grep -q "state = running"; then
  echo ""
  echo "✓ savant-colosseum installed and running"
else
  echo ""
  echo "⚠ savant-colosseum installed but may still be starting..."
fi

LOG_FILE="$HOME_DIR/.savant/colosseum.log"
echo ""
echo "  Binary : $INSTALL_BIN"
echo "  Data   : $HOME_DIR/.savant/colosseum/"
echo "  Logs   : $LOG_FILE"
echo "  Plist  : $PLIST_DEST"
echo "  Server : $SERVER_URL"
echo "  Poll   : every ${POLL_SECONDS}s"
echo ""
echo "  Useful commands:"
echo "    launchctl bootout $LAUNCHD_DOMAIN $PLIST_DEST                  # stop"
echo "    launchctl bootstrap $LAUNCHD_DOMAIN $PLIST_DEST                # start"
echo "    launchctl kickstart -k $LAUNCHD_DOMAIN/$PLIST_NAME             # restart"
echo "    tail -f $LOG_FILE                                              # live logs"
echo "    bash $(dirname "${BASH_SOURCE[0]}")/logs.sh                    # log viewer"
