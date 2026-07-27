#!/bin/sh
set -eu

if [ -f "$HOME/.cargo/env" ]; then
  . "$HOME/.cargo/env"
fi

cd "$(dirname "$0")"
exec cargo run --bin savant-executioner -- "$@"
