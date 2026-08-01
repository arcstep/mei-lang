#!/usr/bin/env bash
# Initialize a directory as a MeiLang v2 workspace (config, stock, deploy scripts).
# 场景钉死：--scenario monorepo|lang-source|installed（见 0608）。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

TARGET_DIR=""
WORKSPACE_ID=""
WORKSPACE_LABEL=""
APP_ID=""
DO_INSTALL=1
RUNTIME="local"
SCENARIO=""
MEI_LANG_ROOT="${MEI_LANG_ROOT:-}"
MEI_ENV_ROOT_OPT="${MEI_ENV_ROOT:-}"
TARGET_TAG="${MEI_RELEASE_TAG:-darwin-arm64-local}"

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
    --mei-env-root) MEI_ENV_ROOT_OPT="$2"; shift 2 ;;
    --mei-env-root=*) MEI_ENV_ROOT_OPT="${1#*=}"; shift ;;
    --scenario) SCENARIO="$2"; shift 2 ;;
    --scenario=*) SCENARIO="${1#*=}"; shift ;;
    --tag) TARGET_TAG="$2"; shift 2 ;;
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

# Infer scenario if unset
if [[ -z "${SCENARIO}" ]]; then
  mono="$(cd "${TARGET_DIR}/../.." 2>/dev/null && pwd || true)"
  if [[ -n "${mono}" && -d "${mono}/mei-lang" && -d "${mono}/mei-env" ]]; then
    SCENARIO="monorepo"
  elif [[ -n "${MEI_LANG_ROOT}" && -f "${MEI_LANG_ROOT}/Cargo.toml" ]]; then
    SCENARIO="lang-source"
  else
    SCENARIO="installed"
  fi
fi

case "${SCENARIO}" in
  monorepo|lang-source|installed) ;;
  *)
    echo "error: --scenario must be monorepo|lang-source|installed" >&2
    exit 1
    ;;
esac

if [[ "${SCENARIO}" != "installed" ]]; then
  if [[ -z "${MEI_LANG_ROOT}" || ! -f "${MEI_LANG_ROOT}/Cargo.toml" ]]; then
    echo "error: set MEI_LANG_ROOT to mei-lang checkout (scenario=${SCENARIO})" >&2
    exit 1
  fi
fi

if [[ -z "${WORKSPACE_ID}" ]]; then
  WORKSPACE_ID="$(basename "${TARGET_DIR}")"
fi

export MEI_LANG_ROOT
export MEI_RUNTIME="${RUNTIME}"
if [[ -n "${MEI_ENV_ROOT_OPT}" ]]; then
  export MEI_ENV_ROOT="${MEI_ENV_ROOT_OPT}"
fi

# shellcheck source=lib.sh
source "${SCRIPT_DIR}/lib.sh"

echo "==> workspace init at ${TARGET_DIR} (id=${WORKSPACE_ID}, scenario=${SCENARIO})"

if [[ "${SCENARIO}" != "installed" ]]; then
  INIT_ARGS=(workspace init --dir "${TARGET_DIR}" --id "${WORKSPACE_ID}")
  if [[ -n "${WORKSPACE_LABEL}" ]]; then
    INIT_ARGS+=(--label "${WORKSPACE_LABEL}")
  fi
  if [[ -n "${APP_ID}" ]]; then
    INIT_ARGS+=(--app "${APP_ID}")
  fi
  run_mei_host_shell "${TARGET_DIR}" "${INIT_ARGS[@]}"
else
  mkdir -p "${TARGET_DIR}"
  if [[ ! -f "${TARGET_DIR}/workspace.json" ]]; then
    echo "error: scenario=installed requires an existing workspace.json (sync from package)" >&2
    exit 1
  fi
fi

echo "==> copy deploy scripts"
mkdir -p "${TARGET_DIR}/deploy"
for f in lib.sh install.sh init.sh fill.sh dev.sh prod.sh build.sh start.sh stop.sh compile.sh reload.sh prebuild.sh status.sh; do
  if [[ -f "${SCRIPT_DIR}/${f}" ]]; then
    cp "${SCRIPT_DIR}/${f}" "${TARGET_DIR}/deploy/${f}"
    chmod +x "${TARGET_DIR}/deploy/${f}"
  fi
done

# Write / merge meiEnv into workspace.json
WS_JSON="${TARGET_DIR}/workspace.json"
MEI_ENV_RESOLVED=""
if [[ -n "${MEI_ENV_ROOT_OPT}" ]]; then
  MEI_ENV_RESOLVED="${MEI_ENV_ROOT_OPT}"
elif [[ "${SCENARIO}" == "monorepo" ]]; then
  MEI_ENV_RESOLVED="$(cd "${TARGET_DIR}/../../mei-env" && pwd)"
else
  MEI_ENV_RESOLVED="${HOME}/.mei-env"
  mkdir -p "${MEI_ENV_RESOLVED}/targets"
fi

if [[ -f "${WS_JSON}" ]] && command -v jq >/dev/null 2>&1; then
  tmp="$(mktemp)"
  jq --arg scenario "${SCENARIO}" --arg tag "${TARGET_TAG}" --arg root "${MEI_ENV_RESOLVED}" \
    '.meiEnv = ((.meiEnv // {}) + {scenario: $scenario, targetTag: $tag, root: $root})' \
    "${WS_JSON}" >"${tmp}" && mv "${tmp}" "${WS_JSON}"
  echo "==> wrote meiEnv into workspace.json (scenario=${SCENARIO})"
fi

# Optional thin cursor rule stub (language authoring; not full mei-docs pack)
if [[ "${SCENARIO}" != "installed" ]]; then
  mkdir -p "${TARGET_DIR}/.cursor/rules"
  cat >"${TARGET_DIR}/.cursor/rules/mei-workspace-local.mdc" <<EOF
---
description: Thin workspace authoring boundary for MeiLang apps (scenario=${SCENARIO})
alwaysApply: true
---

# MeiLang workspace (local)

- 日常入口：\`deploy/fill.sh\` → \`install.sh\` → \`prebuild.sh\` → \`start.sh\`（见 SSOT 0608）。
- 只认 \`deploy/bin\`；不要把 \`mei-lang/target\` 当运行入口。
- 平台工具链在 mei-env（本场景 root=\`${MEI_ENV_RESOLVED}\`）。
EOF
fi

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
  echo "==> fill + install toolchain"
  if [[ "${SCENARIO}" != "installed" ]]; then
    MEI_LANG_ROOT="${MEI_LANG_ROOT}" MEI_ENV_ROOT="${MEI_ENV_RESOLVED}" \
      "${TARGET_DIR}/deploy/fill.sh" --scenario "${SCENARIO}" --tag "${TARGET_TAG}" || true
  fi
  MEI_LANG_ROOT="${MEI_LANG_ROOT:-}" MEI_ENV_ROOT="${MEI_ENV_RESOLVED}" \
    "${TARGET_DIR}/deploy/install.sh" --from env --tag "${TARGET_TAG}" || {
      echo "warn: install skipped/failed; run fill.sh then install.sh manually" >&2
    }
fi

echo "Workspace ready: ${TARGET_DIR}"
echo "Next: cd ${TARGET_DIR} && ./deploy/dev.sh"
