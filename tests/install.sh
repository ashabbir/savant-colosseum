#!/usr/bin/env bash
# Regression test: a failed replacement must leave the existing CLI runnable.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

cp "$REPO_ROOT/install.sh" "$TEMP_DIR/install.sh"
chmod +x "$TEMP_DIR/install.sh"
mkdir -p "$TEMP_DIR/bin" "$TEMP_DIR/mock-bin"
printf '#!/usr/bin/env bash\necho "{\\"data\\":{\\"version\\":\\"old\\"}}"\n' > "$TEMP_DIR/bin/savant-colosseum"
chmod +x "$TEMP_DIR/bin/savant-colosseum"

cat > "$TEMP_DIR/mock-bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
mkdir -p target/release
printf '#!/usr/bin/env bash\necho "{\\"data\\":{\\"version\\":\\"new\\"}}"\n' > target/release/savant-colosseum
chmod +x target/release/savant-colosseum
EOF
chmod +x "$TEMP_DIR/mock-bin/cargo"

cat > "$TEMP_DIR/mock-bin/mv" <<'EOF'
#!/usr/bin/env bash
exit 1
EOF
chmod +x "$TEMP_DIR/mock-bin/mv"

if (
  cd "$TEMP_DIR"
  PATH="$TEMP_DIR/mock-bin:$PATH" SAVANT_COLOSSEUM_BIN_DIR="$TEMP_DIR/bin" bash ./install.sh
); then
  echo "expected installation to fail when the final replacement fails" >&2
  exit 1
fi

[[ "$("$TEMP_DIR/bin/savant-colosseum")" == '{"data":{"version":"old"}}' ]]

rm "$TEMP_DIR/mock-bin/mv"
(
  cd "$TEMP_DIR"
  PATH="$TEMP_DIR/mock-bin:$PATH" SAVANT_COLOSSEUM_BIN_DIR="$TEMP_DIR/bin" bash ./install.sh
)
[[ "$("$TEMP_DIR/bin/savant-colosseum")" == '{"data":{"version":"new"}}' ]]
