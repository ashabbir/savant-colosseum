#!/usr/bin/env bash
# View and interact with savant-colosseum logs.
# Usage:
#   logs.sh            — tail the service log (live follow)
#   logs.sh tail       — tail the service log (live follow)
#   logs.sh last [N]   — show last N lines of the service log (default 100)
#   logs.sh runs       — list all execution run logs
#   logs.sh run <ID>   — view a specific run log by task ID or run UUID
#   logs.sh status     — show launchd service status
#   logs.sh clear      — truncate the service log
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HOME_DIR="$HOME"
SERVICE_LOG="$HOME_DIR/.savant/colosseum.log"
RUN_LOG_DIR="$HOME_DIR/.savant/colosseum/logs"
PLIST_NAME="com.savant.colosseum"
UID_VALUE="$(id -u)"
LAUNCHD_DOMAIN="gui/${UID_VALUE}"
source "$SCRIPT_DIR/scripts/log-service.sh"
source "$SCRIPT_DIR/scripts/log-runs.sh"

# ---------------------------------------------------------------------------
# Dispatch
# ---------------------------------------------------------------------------
case "${1:-tail}" in
  tail)    cmd_tail ;;
  last)    cmd_last "${2:-100}" ;;
  runs)    cmd_runs ;;
  run)     cmd_run "${2:-}" ;;
  status)  cmd_status ;;
  clear)   cmd_clear ;;
  -h|--help|help)
    echo "Usage: logs.sh [command]"
    echo ""
    echo "Commands:"
    echo "  tail           Live-follow the service log (default)"
    echo "  last [N]       Show last N lines of the service log"
    echo "  runs           List all execution run logs"
    echo "  run <ID>       View a specific run log (by task ID or run UUID)"
    echo "  status         Show launchd service status"
    echo "  clear          Truncate the service log"
    ;;
  *)
    echo "Unknown command: $1"
    echo "Run 'logs.sh help' for usage."
    exit 1
    ;;
esac
