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

HOME_DIR="$HOME"
SERVICE_LOG="$HOME_DIR/.savant/colosseum.log"
RUN_LOG_DIR="$HOME_DIR/.savant/colosseum/logs"
PLIST_NAME="com.savant.colosseum"
UID_VALUE="$(id -u)"
LAUNCHD_DOMAIN="gui/${UID_VALUE}"

_ensure_log() {
  if [[ ! -f "$SERVICE_LOG" ]]; then
    echo "No service log found at $SERVICE_LOG"
    echo "Has savant-colosseum been installed? Run: bash install.sh"
    exit 1
  fi
}

cmd_tail() {
  _ensure_log
  echo "═══ savant-colosseum service log (live) ═══"
  echo "    $SERVICE_LOG"
  echo "    Press Ctrl-C to stop"
  echo ""
  tail -f "$SERVICE_LOG"
}

cmd_last() {
  _ensure_log
  local lines="${1:-100}"
  echo "═══ savant-colosseum service log (last $lines lines) ═══"
  tail -n "$lines" "$SERVICE_LOG"
}

cmd_runs() {
  if [[ ! -d "$RUN_LOG_DIR" ]]; then
    echo "No execution run logs found at $RUN_LOG_DIR"
    exit 0
  fi
  echo "═══ Colosseum execution runs ═══"
  echo ""
  printf "%-38s  %-20s  %s\n" "TASK ID" "RUN FILE" "SIZE"
  echo "──────────────────────────────────────  ────────────────────  ──────"
  find "$RUN_LOG_DIR" -name '*.jsonl' -type f 2>/dev/null | sort -r | while read -r f; do
    task_dir="$(basename "$(dirname "$f")")"
    run_file="$(basename "$f")"
    size="$(du -h "$f" | cut -f1 | xargs)"
    printf "%-38s  %-20s  %s\n" "$task_dir" "$run_file" "$size"
  done
}

cmd_run() {
  local query="$1"
  if [[ -z "$query" ]]; then
    echo "Usage: logs.sh run <task-id or run-uuid>"
    exit 1
  fi

  # Search by task ID directory or run UUID filename.
  local found=""
  if [[ -d "$RUN_LOG_DIR/$query" ]]; then
    # Task ID directory — show latest run.
    found="$(ls -t "$RUN_LOG_DIR/$query"/*.jsonl 2>/dev/null | head -1)"
  else
    # Search by UUID in filenames.
    found="$(find "$RUN_LOG_DIR" -name "${query}*" -type f 2>/dev/null | head -1)"
  fi

  if [[ -z "$found" ]]; then
    echo "No run log found matching '$query'"
    echo "Use 'logs.sh runs' to list available runs."
    exit 1
  fi

  echo "═══ Colosseum run log ═══"
  echo "    $found"
  echo ""
  # Pretty print each JSONL line.
  while IFS= read -r line; do
    echo "$line" | python3 -m json.tool 2>/dev/null || echo "$line"
  done < "$found"
}

cmd_status() {
  echo "═══ savant-colosseum service status ═══"
  echo ""
  if launchctl print "$LAUNCHD_DOMAIN/$PLIST_NAME" 2>/dev/null; then
    echo ""
  else
    echo "Service not loaded. Run: bash install.sh"
  fi
}

cmd_clear() {
  _ensure_log
  : > "$SERVICE_LOG"
  echo "✓ Service log cleared: $SERVICE_LOG"
}

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
