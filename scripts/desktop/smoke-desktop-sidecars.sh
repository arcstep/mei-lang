#!/usr/bin/env bash
# Smoke-test Viewer sidecars after collect-desktop-sidecars.sh.
# Does not start Tauri / GUI / network. Shared by desktop-viewer CI and Release.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BIN_DIR="${MEI_LANG_ROOT}/desktop/sidecars/bin"
EXPECTED_VERSION="${MEI_EXPECTED_VERSION:-}"

usage() {
  cat <<'EOF'
Usage: smoke-desktop-sidecars.sh [--bin-dir DIR]

Asserts desktop sidecars contain mei host/runtime/snapshot/compiler + martin,
each responds to --version (martin may use --help), and no libduckdb is present.

Env:
  MEI_EXPECTED_VERSION  If set, mei binary --version output must contain this string.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bin-dir) BIN_DIR="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

EXT=""
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*|Windows_NT) EXT=".exe" ;;
esac

if [[ ! -d "${BIN_DIR}" ]]; then
  echo "error: sidecar bin dir missing: ${BIN_DIR}" >&2
  exit 1
fi

if [[ -z "${EXPECTED_VERSION}" && -f "${MEI_LANG_ROOT}/Cargo.toml" ]]; then
  EXPECTED_VERSION="$(
    python3 - <<'PY' "${MEI_LANG_ROOT}/Cargo.toml"
import re, sys
text = open(sys.argv[1], encoding="utf-8").read()
# Prefer [workspace.package] version when present.
m = re.search(r"(?ms)^\[workspace\.package\]\s*(.*?)^\[", text)
block = m.group(1) if m else text
vm = re.search(r'(?m)^version\s*=\s*"([^"]+)"', block)
print(vm.group(1) if vm else "")
PY
  )"
fi

require_bin() {
  local name="$1"
  local path="${BIN_DIR}/${name}${EXT}"
  if [[ ! -f "${path}" ]]; then
    echo "error: missing sidecar binary: ${path}" >&2
    exit 1
  fi
  if [[ ! -x "${path}" ]] && [[ -z "${EXT}" ]]; then
    echo "error: sidecar binary not executable: ${path}" >&2
    exit 1
  fi
  echo "  ok present ${name}${EXT}"
}

smoke_mei() {
  local name="$1"
  local path="${BIN_DIR}/${name}${EXT}"
  local output
  output="$("${path}" --version 2>&1)" || {
    echo "error: ${name}${EXT} --version failed" >&2
    echo "${output}" >&2
    exit 1
  }
  if [[ -z "${output//[[:space:]]/}" ]]; then
    echo "error: ${name}${EXT} --version produced empty output" >&2
    exit 1
  fi
  if [[ -n "${EXPECTED_VERSION}" && "${output}" != *"${EXPECTED_VERSION}"* ]]; then
    echo "error: ${name}${EXT} --version did not report ${EXPECTED_VERSION}" >&2
    echo "${output}" >&2
    exit 1
  fi
  echo "  ok ${name}${EXT}: ${output%%$'\n'*}"
}

smoke_martin() {
  local path="${BIN_DIR}/martin${EXT}"
  local output
  if output="$("${path}" --version 2>&1)"; then
    if [[ -z "${output//[[:space:]]/}" ]]; then
      echo "error: martin${EXT} --version produced empty output" >&2
      exit 1
    fi
    echo "  ok martin${EXT}: ${output%%$'\n'*}"
    return 0
  fi
  if output="$("${path}" --help 2>&1)"; then
    echo "  ok martin${EXT}: --help succeeded"
    return 0
  fi
  echo "error: martin${EXT} --version/--help failed" >&2
  echo "${output}" >&2
  exit 1
}

echo "==> smoke desktop sidecars in ${BIN_DIR}"
if [[ -n "${EXPECTED_VERSION}" ]]; then
  echo "  expected mei version: ${EXPECTED_VERSION}"
fi

for name in mei-host-shell mei-app-runtime mei-snapshot mei-compiler martin; do
  require_bin "${name}"
done

for name in mei-host-shell mei-app-runtime mei-snapshot mei-compiler; do
  smoke_mei "${name}"
done
smoke_martin

if compgen -G "${BIN_DIR}/libduckdb.*" >/dev/null \
  || [[ -f "${BIN_DIR}/duckdb.dll" ]]; then
  echo "error: libduckdb must not ship (DataFusion query path)" >&2
  ls -la "${BIN_DIR}"/libduckdb* "${BIN_DIR}/duckdb.dll" 2>/dev/null || true
  exit 1
fi
echo "  ok: no libduckdb in sidecars"

echo "==> smoke desktop sidecars passed"
