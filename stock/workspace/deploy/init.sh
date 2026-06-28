#!/usr/bin/env bash
# Initialize a directory as a MeiLang v2 workspace (config, stock, deploy scripts).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

TARGET_DIR=""
WORKSPACE_ID=""
WORKSPACE_LABEL=""
APP_ID=""
DO_INSTALL=1
RUNTIME="cargo"
MEI_LANG_ROOT="${MEI_LANG_ROOT:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir) TARGET_DIR="$2"; shift 2 ;;
    --dir=*) TARGET_DIR="${1#*=}"; shift ;;
    --id) WORKSPACE_ID="$2"; shift 2 ;;
    --id=*) WORKSPACE_ID="${1#*=}"; shift ;;
    --label) WORKSPACE_LABEL="$2"; shift 2 ;;
    --label=*) WORKSPACE_LABEL="${1#*=}"; shift ;;
    --app) APP_ID="$2"; shift 2 ;;
    --app=*) APP_ID="${1#*=}"; shift ;;
    --no-install) DO_INSTALL=0; shift ;;
    --mei-lang-root) MEI_LANG_ROOT="$2"; shift 2 ;;
    --mei-lang-root=*) MEI_LANG_ROOT="${1#*=}"; shift ;;
    --runtime) RUNTIME="$2"; shift 2 ;;
    --runtime=*) RUNTIME="${1#*=}"; shift ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "${TARGET_DIR}" ]]; then
  TARGET_DIR="$(pwd)"
fi
TARGET_DIR="$(cd "${TARGET_DIR}" && pwd)"

if [[ -z "${MEI_LANG_ROOT}" ]]; then
  if [[ -f "${SCRIPT_DIR}/../../../Cargo.toml" ]]; then
    MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
  elif [[ -f "${TARGET_DIR}/../../mei-lang/Cargo.toml" ]]; then
    MEI_LANG_ROOT="$(cd "${TARGET_DIR}/../../mei-lang" && pwd)"
  fi
fi

if [[ -z "${MEI_LANG_ROOT}" || ! -f "${MEI_LANG_ROOT}/Cargo.toml" ]]; then
  echo "error: set MEI_LANG_ROOT to mei-lang checkout" >&2
  exit 1
fi

if [[ -z "${WORKSPACE_ID}" ]]; then
  WORKSPACE_ID="$(basename "${TARGET_DIR}")"
fi

export MEI_LANG_ROOT
export MEI_RUNTIME="${RUNTIME}"

# shellcheck source=lib.sh
source "${SCRIPT_DIR}/lib.sh"

echo "==> workspace init at ${TARGET_DIR} (id=${WORKSPACE_ID})"

INIT_ARGS=(workspace init --dir "${TARGET_DIR}" --id "${WORKSPACE_ID}")
if [[ -n "${WORKSPACE_LABEL}" ]]; then
  INIT_ARGS+=(--label "${WORKSPACE_LABEL}")
fi
if [[ -n "${APP_ID}" ]]; then
  INIT_ARGS+=(--app "${APP_ID}")
fi
run_mei_host_shell "${TARGET_DIR}" "${INIT_ARGS[@]}"

echo "==> copy deploy scripts"
mkdir -p "${TARGET_DIR}/deploy"
for f in lib.sh install.sh init.sh start.sh stop.sh compile.sh reload.sh prebuild.sh status.sh; do
  if [[ -f "${SCRIPT_DIR}/${f}" ]]; then
    cp "${SCRIPT_DIR}/${f}" "${TARGET_DIR}/deploy/${f}"
    chmod +x "${TARGET_DIR}/deploy/${f}"
  fi
done

if [[ -f "${SCRIPT_DIR}/gitignore.snippet" ]]; then
  if [[ -f "${TARGET_DIR}/.gitignore" ]]; then
    if ! grep -q 'deploy/bin/' "${TARGET_DIR}/.gitignore" 2>/dev/null; then
      echo "" >> "${TARGET_DIR}/.gitignore"
      cat "${SCRIPT_DIR}/gitignore.snippet" >> "${TARGET_DIR}/.gitignore"
    fi
  else
    cp "${SCRIPT_DIR}/gitignore.snippet" "${TARGET_DIR}/.gitignore"
  fi
fi

if [[ "${DO_INSTALL}" -eq 1 ]]; then
  echo "==> install local binaries"
  MEI_LANG_ROOT="${MEI_LANG_ROOT}" "${TARGET_DIR}/deploy/install.sh"
fi

echo "Workspace ready: ${TARGET_DIR}"
echo "Next: cd ${TARGET_DIR} && ./deploy/start.sh"
