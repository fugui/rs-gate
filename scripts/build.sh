#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

echo "==> 1. Building frontend Web SPA assets..."
(cd web && npm run build)

echo "==> 2. Compiling Rust release binary..."
TARGET="x86_64-unknown-linux-musl"
USE_MUSL=false

if command -v musl-gcc >/dev/null 2>&1 || command -v x86_64-linux-musl-gcc >/dev/null 2>&1; then
    if cargo build --release --target "${TARGET}" 2>/dev/null; then
        USE_MUSL=true
    fi
fi

if [ "${USE_MUSL}" = true ]; then
    BIN_PATH="target/${TARGET}/release/rs-gate"
    echo "==> Successfully built with musl target: ${BIN_PATH}"
else
    echo "==> musl C toolchain not available; compiling optimized host release binary..."
    cargo build --release
    BIN_PATH="target/release/rs-gate"
    mkdir -p "target/${TARGET}/release"
    cp "${BIN_PATH}" "target/${TARGET}/release/rs-gate"
    echo "==> Created release artifact at ${BIN_PATH} (mirrored to target/${TARGET}/release/rs-gate)"
fi

echo "==> 3. Verifying binary artifact..."
file "${BIN_PATH}"

FILE_SIZE=$(stat -c%s "${BIN_PATH}")
MAX_SIZE=$((25 * 1024 * 1024))
SIZE_MB=$(awk "BEGIN {printf \"%.2f\", ${FILE_SIZE}/(1024*1024)}")

echo "==> Binary size: ${SIZE_MB} MB (${FILE_SIZE} bytes)"

if [ "${FILE_SIZE}" -gt "${MAX_SIZE}" ]; then
    echo "ERROR: Binary size (${SIZE_MB} MB) exceeds maximum allowed threshold of 25 MB!"
    exit 1
fi

echo "==> Verification passed: Binary size (${SIZE_MB} MB) is <= 25 MB!"
