#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  cat <<'EOF'
Usage:
  bash infra/board/nucleo-f103-firmware/scripts/build.sh <blink|modbus_slave>

Examples:
  bash infra/board/nucleo-f103-firmware/scripts/build.sh blink
  bash infra/board/nucleo-f103-firmware/scripts/build.sh modbus_slave
EOF
  exit 2
fi

BIN="$1"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [[ "$BIN" != "blink" && "$BIN" != "modbus_slave" ]]; then
  echo "Unsupported binary: $BIN"
  exit 2
fi

echo "== Build firmware =="
echo "Project: $ROOT_DIR"
echo "Binary:  $BIN"

rustup target add thumbv7m-none-eabi >/dev/null
rustup component add llvm-tools-preview >/dev/null

if ! command -v cargo-objcopy >/dev/null 2>&1; then
  echo "cargo-objcopy not found; installing cargo-binutils..."
  cargo install cargo-binutils --locked
fi

pushd "$ROOT_DIR" >/dev/null

cargo build \
  --release \
  --target thumbv7m-none-eabi \
  --bin "$BIN"

cargo objcopy \
  --release \
  --target thumbv7m-none-eabi \
  --bin "$BIN" \
  -- \
  -O binary \
  "target/thumbv7m-none-eabi/release/${BIN}.bin"

popd >/dev/null

echo
echo "Output:"
echo "  $ROOT_DIR/target/thumbv7m-none-eabi/release/${BIN}.elf"
echo "  $ROOT_DIR/target/thumbv7m-none-eabi/release/${BIN}.bin"
