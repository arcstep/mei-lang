#!/usr/bin/env bash
# Initialize a directory as a MeiLang v2 workspace (config + thin deploy surface).
# Not part of the daily deploy/ three-script surface — called by mei.sh init / workspace init.
# Prefer: mei-lang/scripts/env/mei.sh init|workspace init
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_STOCK="${SCRIPT_DIR}/deploy"

TARGET_DIR=""
WORKSPACE_ID=""
WORKSPACE_LABEL=""
APP_ID=""
RUNTIME="mei-env"
MEI_LANG_ROOT="${MEI_LANG_ROOT:-}"
MEI_ENV_ROOT_OPT="${MEI_ENV_ROOT:-}"
TARGET_TAG="${MEI_RELEASE_TAG:-}"
VERSION_PIN="${MEI_RELEASE_VERSION:-}"
FROM_BUNDLE=0
LEGACY_SCENARIO=""

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
    --no-install) shift ;; # deprecated no-op
    --mei-lang-root|--source) MEI_LANG_ROOT="$2"; shift 2 ;;
    --mei-lang-root=*|--source=*) MEI_LANG_ROOT="${1#*=}"; shift ;;
    --mei-env-root|--env) MEI_ENV_ROOT_OPT="$2"; shift 2 ;;
    --mei-env-root=*|--env=*) MEI_ENV_ROOT_OPT="${1#*=}"; shift ;;
    --from-bundle) FROM_BUNDLE=1; shift ;;
    --scenario)
      LEGACY_SCENARIO="$2"
      echo "warn: --scenario is deprecated on init-workspace; use --source/--env/--from-bundle" >&2
      shift 2
      ;;
    --scenario=*)
      LEGACY_SCENARIO="${1#*=}"
      echo "warn: --scenario is deprecated on init-workspace; use --source/--env/--from-bundle" >&2
      shift
      ;;
    --tag) TARGET_TAG="$2"; shift 2 ;;
    --version) VERSION_PIN="$2"; shift 2 ;;
    --runtime) RUNTIME="$2"; shift 2 ;;
    --runtime=*) RUNTIME="${1#*=}"; shift ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

if [[ -z "${TARGET_DIR}" ]]; then
  TARGET_DIR="$(pwd)"
fi
mkdir -p "${TARGET_DIR}"
TARGET_DIR="$(cd "${TARGET_DIR}" && pwd)"

if [[ -z "${MEI_LANG_ROOT}" ]]; then
  if [[ -f "${SCRIPT_DIR}/../../Cargo.toml" ]]; then
    MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
  elif [[ -f "${TARGET_DIR}/../../mei-lang/Cargo.toml" ]]; then
    MEI_LANG_ROOT="$(cd "${TARGET_DIR}/../../mei-lang" && pwd)"
  fi
fi

# Legacy scenario → flags
case "${LEGACY_SCENARIO}" in
  installed) FROM_BUNDLE=1 ;;
  monorepo|lang-source|"") ;;
  *)
    echo "error: unknown deprecated --scenario=${LEGACY_SCENARIO}" >&2
    exit 1
    ;;
esac

BINARY_ONLY=0
if [[ "${FROM_BUNDLE}" -eq 1 ]] || [[ -z "${MEI_LANG_ROOT}" || ! -f "${MEI_LANG_ROOT}/Cargo.toml" ]]; then
  if [[ "${FROM_BUNDLE}" -eq 1 ]] || [[ -z "${MEI_LANG_ROOT}" ]]; then
    BINARY_ONLY=1
  fi
fi

if [[ "${BINARY_ONLY}" -eq 0 ]]; then
  if [[ -z "${MEI_LANG_ROOT}" || ! -f "${MEI_LANG_ROOT}/Cargo.toml" ]]; then
    echo "error: set --source / MEI_LANG_ROOT to mei-lang checkout, or pass --from-bundle" >&2
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

# shellcheck source=deploy/lib.sh
source "${DEPLOY_STOCK}/lib.sh"

MEI_ENV_RESOLVED=""
if [[ -n "${MEI_ENV_ROOT_OPT}" ]]; then
  MEI_ENV_RESOLVED="${MEI_ENV_ROOT_OPT}"
else
  mono="$(cd "${TARGET_DIR}/../.." 2>/dev/null && pwd || true)"
  if [[ -n "${mono}" && -d "${mono}/mei-lang" && -d "${mono}/mei-env" ]]; then
    MEI_ENV_RESOLVED="$(cd "${mono}/mei-env" && pwd)"
  else
    MEI_ENV_RESOLVED="${HOME}/.mei-env"
    mkdir -p "${MEI_ENV_RESOLVED}/targets"
  fi
fi
export MEI_ENV_ROOT="${MEI_ENV_RESOLVED}"

if [[ -z "${TARGET_TAG}" ]]; then
  TARGET_TAG="$(default_local_target_tag)"
fi

echo "==> workspace init at ${TARGET_DIR} (id=${WORKSPACE_ID})"

echo "==> copy deploy thin entries (three shells; lib/impl stay in stock)"
mkdir -p "${TARGET_DIR}/deploy"
for f in build-app.sh start-host.sh stop-host.sh; do
  if [[ -f "${DEPLOY_STOCK}/${f}" ]]; then
    cp "${DEPLOY_STOCK}/${f}" "${TARGET_DIR}/deploy/${f}"
    chmod +x "${TARGET_DIR}/deploy/${f}"
  fi
done
# Drop legacy local copies
rm -f "${TARGET_DIR}/deploy/lib.sh" "${TARGET_DIR}/deploy/build-env.sh" "${TARGET_DIR}/deploy/env-build.sh"
rm -rf "${TARGET_DIR}/deploy/impl"

write_minimal_workspace_json() {
  mkdir -p "${TARGET_DIR}/apps"
  local label_json="${WORKSPACE_LABEL:-${WORKSPACE_ID}}"
  local default_app_json="${APP_ID:-}"
  cat >"${TARGET_DIR}/workspace.json" <<EOF
{
  "workspace": {
    "id": "${WORKSPACE_ID}",
    "label": "${label_json}",
    "defaultApp": "${default_app_json}"
  }
}
EOF
  echo "==> wrote minimal workspace.json"
}

write_mei_env_into_workspace_json() {
  local ws_json="${TARGET_DIR}/workspace.json"
  [[ -f "${ws_json}" ]] || return 0
  if command -v jq >/dev/null 2>&1; then
    local tmp
    tmp="$(mktemp)"
    jq --arg tag "${TARGET_TAG}" --arg root "${MEI_ENV_RESOLVED}" --arg ver "${VERSION_PIN}" '
      .meiEnv = ((.meiEnv // {})
        | .root = $root
        | .targetTag = $tag
        | (if $ver != "" then .version = $ver else . end)
        | del(.scenario))
    ' "${ws_json}" >"${tmp}" && mv "${tmp}" "${ws_json}"
    echo "==> wrote meiEnv into workspace.json (root=${MEI_ENV_RESOLVED} tag=${TARGET_TAG})"
  elif command -v python3 >/dev/null 2>&1; then
    python3 - "${ws_json}" "${TARGET_TAG}" "${MEI_ENV_RESOLVED}" "${VERSION_PIN}" <<'PY'
import json, sys
path, tag, root, ver = sys.argv[1:5]
with open(path, encoding="utf-8") as f:
    data = json.load(f)
mei = dict(data.get("meiEnv") or {})
mei.pop("scenario", None)
mei["root"] = root
mei["targetTag"] = tag
if ver:
    mei["version"] = ver
data["meiEnv"] = mei
with open(path, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
    f.write("\n")
PY
    echo "==> wrote meiEnv into workspace.json via python"
  else
    echo "warn: jq/python3 missing; meiEnv not merged into workspace.json" >&2
  fi
}

# Pin version before host-shell: resolve_bin_path refuses unpinned meiEnv.version.
if [[ -z "${VERSION_PIN}" ]]; then
  VERSION_PIN="$(latest_version_under_tag "${MEI_ENV_RESOLVED}" "${TARGET_TAG}" || true)"
fi
if [[ ! -f "${TARGET_DIR}/workspace.json" ]]; then
  write_minimal_workspace_json
fi
write_mei_env_into_workspace_json

if [[ "${BINARY_ONLY}" -eq 0 ]]; then
  INIT_ARGS=(workspace init --dir "${TARGET_DIR}" --id "${WORKSPACE_ID}")
  if [[ -n "${WORKSPACE_LABEL}" ]]; then
    INIT_ARGS+=(--label "${WORKSPACE_LABEL}")
  fi
  if [[ -n "${APP_ID}" ]]; then
    INIT_ARGS+=(--app "${APP_ID}")
  fi
  # Keep stderr visible so cold-start failures are diagnosable.
  if ! run_mei_host_shell "${TARGET_DIR}" "${INIT_ARGS[@]}"; then
    echo "warn: mei-host-shell workspace init failed; keeping workspace.json + deploy shells" >&2
  fi
fi
# Re-merge meiEnv in case host-shell rewrote workspace.json without it.
write_mei_env_into_workspace_json

if [[ "${BINARY_ONLY}" -eq 0 ]]; then
  if mkdir -p "${TARGET_DIR}/.cursor/rules" 2>/dev/null; then
    cat >"${TARGET_DIR}/.cursor/rules/mei-workspace-local.mdc" <<EOF
---
description: Thin workspace authoring boundary for MeiLang apps
alwaysApply: true
---

# MeiLang workspace (local)

- 冷启动 / 灌 env：\`mei-lang/scripts/env/mei.sh init\` / \`mei.sh env build\`
- 日常：\`deploy/build-app.sh\` → \`start-host.sh\` / \`stop-host.sh\`（0608）
- \`lib.sh\` / \`impl/\` 真源在 mei-lang stock；工作区只留三个薄入口
- 平台工具链 root=\`${MEI_ENV_RESOLVED}\`
EOF
  fi
fi

if [[ -f "${DEPLOY_STOCK}/gitignore.snippet" ]]; then
  if [[ -f "${TARGET_DIR}/.gitignore" ]]; then
    if ! grep -q 'deploy/bin/' "${TARGET_DIR}/.gitignore" 2>/dev/null; then
      echo "" >> "${TARGET_DIR}/.gitignore"
      cat "${DEPLOY_STOCK}/gitignore.snippet" >> "${TARGET_DIR}/.gitignore"
    fi
  else
    cp "${DEPLOY_STOCK}/gitignore.snippet" "${TARGET_DIR}/.gitignore"
  fi
fi

echo "Workspace ready: ${TARGET_DIR}"
echo "Next: cd ${TARGET_DIR} && ./deploy/build-app.sh && ./deploy/start-host.sh"
