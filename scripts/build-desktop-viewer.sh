#!/usr/bin/env bash
# One-shot Mei Viewer build / dev (no directory hopping).
# See docs/mei-lang-v2/05-host/0541-desktop-viewer-implementation-plan.md
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DESKTOP_DIR="${MEI_LANG_ROOT}/desktop"

MODE="build"          # build | dev
PROFILE="release"     # release | debug
SKIP_COLLECT=0
SKIP_NPM_INSTALL=0
COLLECT_EXTRA=()

usage() {
  cat <<'EOF'
Usage: build-desktop-viewer.sh [options]

From mei-lang root (or any cwd):
  ./scripts/build-desktop-viewer.sh              # collect sidecars + npm run build
  ./scripts/build-desktop-viewer.sh --dev        # collect (debug) + npm run dev
  ./scripts/build-desktop-viewer.sh --skip-collect   # only tauri build/dev

Options:
  --build              Release package (default)
  --dev                Hot-run via tauri dev
  --release            Sidecar profile=release (default for --build)
  --debug              Sidecar profile=debug (default for --dev)
  --skip-collect       Do not run collect-desktop-sidecars.sh
  --skip-npm-install   Skip npm install if node_modules exists
  --skip-assets        Forwarded to collect script
  --skip-build         Forwarded to collect (reuse existing cargo bins)
  -h, --help           Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build) MODE="build"; shift ;;
    --dev) MODE="dev"; PROFILE="debug"; shift ;;
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
    --skip-collect) SKIP_COLLECT=1; shift ;;
    --skip-npm-install) SKIP_NPM_INSTALL=1; shift ;;
    --skip-assets) COLLECT_EXTRA+=(--skip-assets); shift ;;
    --skip-build) COLLECT_EXTRA+=(--skip-build); shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

if [[ ! -d "${DESKTOP_DIR}" ]]; then
  echo "desktop/ not found under ${MEI_LANG_ROOT}" >&2
  exit 1
fi

# Avoid nested CARGO_TARGET_DIR mistakes when invoking from desktop/
unset CARGO_TARGET_DIR || true

if [[ "${SKIP_COLLECT}" != "1" ]]; then
  echo "==> collect sidecars (profile=${PROFILE})"
  "${MEI_LANG_ROOT}/scripts/collect-desktop-sidecars.sh" "--${PROFILE}" "${COLLECT_EXTRA[@]+"${COLLECT_EXTRA[@]}"}"
else
  echo "==> skip collect"
fi

cd "${DESKTOP_DIR}"
if [[ "${SKIP_NPM_INSTALL}" != "1" ]] || [[ ! -d node_modules ]]; then
  echo "==> npm install"
  npm install
fi

case "${MODE}" in
  build)
    echo "==> npm run build"
    npm run build
    echo
    if [[ "$(uname -s)" == "Darwin" ]]; then
      echo "OPEN: ${DESKTOP_DIR}/dist/mei-viewer.app"
      echo "      open \"${DESKTOP_DIR}/dist/mei-viewer.app\""
      echo "ZIP:  ${DESKTOP_DIR}/dist/"
      echo "RAW:  ${DESKTOP_DIR}/src-tauri/target/release/bundle/macos/mei-viewer.app"
    else
      echo "DIST: ${DESKTOP_DIR}/dist/"
    fi
    ;;
  dev)
    echo "==> npm run dev"
    npm run dev
    ;;
  *)
    echo "unknown mode: ${MODE}" >&2
    exit 1
    ;;
esac
