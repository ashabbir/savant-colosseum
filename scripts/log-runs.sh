cmd_runs() {
  if [[ ! -d "$RUN_LOG_DIR" ]]; then
    echo "No execution run logs found at $RUN_LOG_DIR"
    exit 0
  fi
  echo "═══ Colosseum execution runs ═══"
  echo ""
  printf "%-38s  %-20s  %s\n" "TASK ID" "RUN FILE" "SIZE"
  echo "──────────────────────────────────────  ────────────────────  ──────"
  find "$RUN_LOG_DIR" -name '*.jsonl' -type f 2>/dev/null | sort -r | while read -r file; do
    task_dir="$(basename "$(dirname "$file")")"
    run_file="$(basename "$file")"
    size="$(du -h "$file" | cut -f1 | xargs)"
    printf "%-38s  %-20s  %s\n" "$task_dir" "$run_file" "$size"
  done
}

cmd_run() {
  local query="$1"
  if [[ -z "$query" ]]; then
    echo "Usage: logs.sh run <task-id or run-uuid>"
    exit 1
  fi
  local found=""
  if [[ -d "$RUN_LOG_DIR/$query" ]]; then
    found="$(ls -t "$RUN_LOG_DIR/$query"/*.jsonl 2>/dev/null | head -1)"
  else
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
  while IFS= read -r line; do
    echo "$line" | python3 -m json.tool 2>/dev/null || echo "$line"
  done < "$found"
}
