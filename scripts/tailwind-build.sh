#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="${ROOT_DIR}/.tailwind"
VERSION="v3.4.17"

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}:${ARCH}" in
  Darwin:arm64)
    PLATFORM="macos-arm64"
    ;;
  Darwin:x86_64)
    PLATFORM="macos-x64"
    ;;
  Linux:x86_64)
    PLATFORM="linux-x64"
    ;;
  Linux:aarch64)
    PLATFORM="linux-arm64"
    ;;
  *)
    echo "unsupported platform: ${OS} ${ARCH}" >&2
    exit 1
    ;;
esac

mkdir -p "${BIN_DIR}"
CLI_PATH="${BIN_DIR}/tailwindcss-${PLATFORM}"

if [[ ! -x "${CLI_PATH}" ]]; then
  URL="https://github.com/tailwindlabs/tailwindcss/releases/download/${VERSION}/tailwindcss-${PLATFORM}"
  echo "downloading tailwindcss ${VERSION} (${PLATFORM})..."
  curl -fsSL "${URL}" -o "${CLI_PATH}"
  chmod +x "${CLI_PATH}"
fi

TAILWIND_ARGS=(
  -c "${ROOT_DIR}/tailwind.config.js"
  -i "${ROOT_DIR}/host-shell/app/assets/tailwind-input.css"
  -o "${ROOT_DIR}/host-shell/app/assets/tailwind.css"
)

if [[ "${1:-}" == "--watch" ]]; then
  exec "${CLI_PATH}" "${TAILWIND_ARGS[@]}" --watch
fi

exec "${CLI_PATH}" "${TAILWIND_ARGS[@]}" --minify
