#!/usr/bin/env bash
# Build and install the Savant Colosseum CLI for the current user (macOS/Linux).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="savant-colosseum"
INSTALL_DIR="${SAVANT_COLOSSEUM_BIN_DIR:-$HOME/.local/bin}"
INSTALL_PATH="$INSTALL_DIR/$BIN_NAME"

if [[ "$(uname -s)" != "Darwin" && "$(uname -s)" != "Linux" ]]; then
  echo "ERROR: savant-colosseum supports macOS and Linux only." >&2
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "ERROR: cargo is required to build savant-colosseum. Install Rust and retry." >&2
  exit 1
fi
if ! mkdir -p "$INSTALL_DIR"; then
  echo "ERROR: cannot create install directory: $INSTALL_DIR" >&2
  exit 1
fi

echo "Building release binary..."
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"

if [[ -e "$INSTALL_PATH" ]]; then
  echo "Upgrading existing installation: $INSTALL_PATH"
fi
install -m 755 "$SCRIPT_DIR/target/release/$BIN_NAME" "$INSTALL_PATH"

INSTALLED_VERSION="$($INSTALL_PATH --version | sed -n 's/.*"version":"\([^"]*\)".*/\1/p')"
if [[ -z "$INSTALLED_VERSION" ]]; then
  echo "ERROR: installed binary did not report a version: $INSTALL_PATH" >&2
  exit 1
fi
echo "Installed $BIN_NAME $INSTALLED_VERSION at $INSTALL_PATH"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "Add this directory to PATH: export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
echo "Worker logs and registry are retained at: ${SAVANT_EXECUTIONER_HOME:-$HOME/.savant/colosseum}/workers"
