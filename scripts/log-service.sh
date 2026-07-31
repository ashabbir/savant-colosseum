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
