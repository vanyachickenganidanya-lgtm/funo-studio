#!/usr/bin/env sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

if [ -x "$ROOT/funo" ]; then
  CLI="$ROOT/funo"
elif [ -x "$ROOT/src-tauri/target/release/funo" ]; then
  CLI="$ROOT/src-tauri/target/release/funo"
else
  if ! command -v cargo >/dev/null 2>&1; then
    echo "Ошибка: установите Rust или положите готовый бинарник funo рядом с проектом." >&2
    exit 1
  fi
  echo "Собираю Funo CLI…"
  cargo build --release --locked --manifest-path "$ROOT/src-tauri/Cargo.toml" --bin funo
  CLI="$ROOT/src-tauri/target/release/funo"
fi

"$CLI" setup
printf '\nГотово. Откройте новый терминал и проверьте: funo --version\n'
