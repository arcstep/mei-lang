#!/usr/bin/env bash
# Workspace deploy runtime: PROFILE (debug|release); binaries from mei-env/targets (no deploy/bin).
# Daily surface: build-app.sh / start-host.sh / stop-host.sh (SSOT 0608).
# Platform fill: mei-lang/scripts/env/mei.sh env build|init
# meiEnv.version must be pinned (no silent latest fallback).
set -euo pipefail

PROFILE="${MEI_PROFILE:-debug}"
SOURCE="${MEI_SOURCE:-mei-env}"
RUNTIME="${MEI_RUNTIME:-mei-env}"

resolve_mei_lang_root() {
  local workspace_root="$1"
  local root="${MEI_LANG_ROOT:-${workspace_root}/../../mei-lang}"
  # Source checkout (dev / fill) or clean package (share/mei → mei-package).
  if [[ -f "${root}/Cargo.toml" || -d "${root}/app/assets" || -d "${root}/stock" ]]; then
    printf '%s' "${root}"
    return 0
  fi
  echo "error: mei-lang not found at ${root} (set MEI_LANG_ROOT)" >&2
  return 1
}

# 0608: MEI_ENV_ROOT → workspace.json meiEnv.root → monorepo mei-env → ~/.mei-env
resolve_mei_env_root() {
  local workspace_root="$1"
  local root="" configured=""

  if [[ -n "${MEI_RELEASE_ROOT:-}" && -z "${MEI_ENV_ROOT:-}" ]]; then
    MEI_ENV_ROOT="${MEI_RELEASE_ROOT}"
  fi
  if [[ -n "${MEI_ENV_ROOT:-}" ]]; then
    root="${MEI_ENV_ROOT}"
  else
    configured="$(read_workspace_mei_env_root "${workspace_root}" || true)"
    if [[ -n "${configured}" ]]; then
      root="${configured}"
    else
      local mono
      mono="$(cd "${workspace_root}/../.." 2>/dev/null && pwd || true)"
      if [[ -n "${mono}" && -d "${mono}/mei-lang" && -d "${mono}/mei-env" ]]; then
        root="${mono}/mei-env"
      else
        root="${HOME}/.mei-env"
        mkdir -p "${root}/targets"
      fi
    fi
  fi
  if [[ ! -d "${root}" ]]; then
    echo "error: mei-env not found at ${root} (set MEI_ENV_ROOT or create ~/.mei-env)" >&2
    return 1
  fi
  printf '%s' "${root}"
}

# Deprecated name
resolve_mei_release_root() {
  resolve_mei_env_root "$@"
}

read_workspace_mei_env_field() {
  local workspace_root="$1"
  local field="$2"
  local ws_json="${workspace_root}/workspace.json"
  [[ -f "${ws_json}" ]] || return 1
  if command -v jq >/dev/null 2>&1; then
    local v
    # Prefer simple path: CentOS jq shim may not support --arg / [$f] rewrite forms.
    v="$(jq -r ".meiEnv.${field} // empty" "${ws_json}" 2>/dev/null || true)"
    if [[ -z "${v}" || "${v}" == "null" ]]; then
      v="$(jq -r --arg f "${field}" '.meiEnv[$f] // empty' "${ws_json}" 2>/dev/null || true)"
    fi
    if [[ -n "${v}" && "${v}" != "null" ]]; then
      printf '%s' "${v}"
      return 0
    fi
  fi
  local py=""
  for c in python3 python python2; do
    if command -v "${c}" >/dev/null 2>&1; then py="${c}"; break; fi
  done
  if [[ -n "${py}" ]]; then
    local v
    v="$("${py}" - "${ws_json}" "${field}" <<'PY'
from __future__ import print_function
import json, sys
path, field = sys.argv[1:3]
data = json.load(open(path))
mei = data.get("meiEnv") or {}
val = mei.get(field) if isinstance(mei, dict) else None
if val is None or val == "":
    raise SystemExit(1)
print(val)
PY
)" || true
    if [[ -n "${v}" && "${v}" != "null" ]]; then
      printf '%s' "${v}"
      return 0
    fi
  fi
  return 1
}

# workspace.json#workspace.<field> (port | listenHost)
read_workspace_profile_field() {
  local workspace_root="$1"
  local field="$2"
  local ws_json="${workspace_root}/workspace.json"
  [[ -f "${ws_json}" ]] || return 1
  if command -v jq >/dev/null 2>&1; then
    local v
    v="$(jq -r --arg f "${field}" '.workspace[$f] // empty' "${ws_json}" 2>/dev/null || true)"
    if [[ -n "${v}" && "${v}" != "null" ]]; then
      printf '%s' "${v}"
      return 0
    fi
  elif command -v python3 >/dev/null 2>&1; then
    local v
    v="$(python3 - "${ws_json}" "${field}" <<'PY'
import json, sys
path, field = sys.argv[1:3]
with open(path, encoding="utf-8") as f:
    data = json.load(f)
ws = data.get("workspace") or {}
val = ws.get(field)
if val is None or val == "":
    raise SystemExit(1)
print(val)
PY
)" || return 1
    if [[ -n "${v}" ]]; then
      printf '%s' "${v}"
      return 0
    fi
  fi
  return 1
}

# Priority: MEI_PORT → workspace.port → 9527（CLI --port 在调用方覆盖）
default_workspace_serve_port() {
  local workspace_root="$1"
  local port=""
  if [[ -n "${MEI_PORT:-}" ]]; then
    printf '%s' "${MEI_PORT}"
    return 0
  fi
  port="$(read_workspace_profile_field "${workspace_root}" "port" 2>/dev/null || true)"
  printf '%s' "${port:-9527}"
}

# Priority: MEI_SERVE_HOST → workspace.listenHost → 127.0.0.1（CLI --host 在调用方覆盖）
default_workspace_serve_host() {
  local workspace_root="$1"
  local host=""
  if [[ -n "${MEI_SERVE_HOST:-}" ]]; then
    printf '%s' "${MEI_SERVE_HOST}"
    return 0
  fi
  host="$(read_workspace_profile_field "${workspace_root}" "listenHost" 2>/dev/null || true)"
  printf '%s' "${host:-127.0.0.1}"
}

read_workspace_mei_env_root() {
  read_workspace_mei_env_field "$1" "root"
}

read_workspace_mei_env_target_tag() {
  read_workspace_mei_env_field "$1" "targetTag"
}

read_workspace_mei_env_version() {
  read_workspace_mei_env_field "$1" "version"
}

read_workspace_mei_env_scenario() {
  read_workspace_mei_env_field "$1" "scenario"
}

write_workspace_mei_env_fields() {
  local workspace_root="$1"
  local tag="${2:-}"
  local version="${3:-}"
  local ws_json="${workspace_root}/workspace.json"
  [[ -f "${ws_json}" ]] || return 1
  if command -v jq >/dev/null 2>&1; then
    local tmp
    tmp="$(mktemp)"
    jq --arg tag "${tag}" --arg ver "${version}" '
      .meiEnv = ((.meiEnv // {})
        | (if $tag != "" then .targetTag = $tag else . end)
        | (if $ver != "" then .version = $ver else . end))
    ' "${ws_json}" >"${tmp}" && mv "${tmp}" "${ws_json}"
    return 0
  fi
  if command -v python3 >/dev/null 2>&1; then
    python3 - "${ws_json}" "${tag}" "${version}" <<'PY'
import json, sys
path, tag, ver = sys.argv[1:4]
with open(path, encoding="utf-8") as f:
    data = json.load(f)
mei = dict(data.get("meiEnv") or {})
if tag:
    mei["targetTag"] = tag
if ver:
    mei["version"] = ver
data["meiEnv"] = mei
with open(path, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
    f.write("\n")
PY
    return 0
  fi
  return 1
}

default_local_target_tag() {
  local sys mach
  sys="$(/usr/bin/uname -s 2>/dev/null || uname -s)"
  mach="$(/usr/bin/uname -m 2>/dev/null || uname -m)"
  case "${sys}-${mach}" in
    Darwin-arm64) printf '%s' "darwin-arm64-local" ;;
    Darwin-x86_64) printf '%s' "darwin-x86_64-local" ;;
    Linux-x86_64) printf '%s' "linux-x86_64-local" ;;
    Linux-aarch64) printf '%s' "linux-aarch64-local" ;;
    MINGW*|MSYS*|CYGWIN*) printf '%s' "windows-x86_64-local" ;;
    *) printf '%s' "local" ;;
  esac
}

resolve_mei_env_target_tag() {
  local workspace_root="$1"
  local tag="${MEI_RELEASE_TAG:-}"
  if [[ -z "${tag}" ]]; then
    tag="$(read_workspace_mei_env_target_tag "${workspace_root}" || true)"
  fi
  if [[ -z "${tag}" ]]; then
    tag="$(default_local_target_tag)"
  fi
  printf '%s' "${tag}"
}

# Newest version dir under targets/<tag>/ that has host-shell (by mtime).
latest_version_under_tag() {
  local mei_env_root="$1"
  local tag="$2"
  local tag_root="${mei_env_root}/targets/${tag}"
  [[ -d "${tag_root}" ]] || return 1
  local newest="" newest_mtime=0 d mtime
  for d in "${tag_root}"/mei-lang-* "${tag_root}"/*; do
    [[ -d "${d}" ]] || continue
    if [[ -x "${d}/v2/bin/mei-host-shell" || -x "${d}/bin/mei-host-shell" ]]; then
      mtime="$(stat -f '%m' "${d}" 2>/dev/null || stat -c '%Y' "${d}" 2>/dev/null || echo 0)"
      if [[ "${mtime}" -ge "${newest_mtime}" ]]; then
        newest_mtime="${mtime}"
        newest="$(basename "${d}")"
      fi
    fi
  done
  [[ -n "${newest}" ]] || return 1
  printf '%s' "${newest}"
}

find_release_binary() {
  local bundle_root="$1"
  local name="$2"
  local candidate
  for candidate in \
    "${bundle_root}/v2/bin/${name}" \
    "${bundle_root}/bin/${name}" \
    "${bundle_root}/v2/${name}" \
    "${bundle_root}/${name}"; do
    if [[ -x "${candidate}" ]]; then
      printf '%s' "${candidate}"
      return 0
    fi
  done
  return 1
}

resolve_mei_env_bundle_root() {
  local workspace_root="$1"
  local mei_env_root tag version bundle_root
  mei_env_root="$(resolve_mei_env_root "${workspace_root}")"
  tag="$(resolve_mei_env_target_tag "${workspace_root}")"
  version="${MEI_RELEASE_VERSION:-}"
  if [[ -z "${version}" ]]; then
    version="$(read_workspace_mei_env_version "${workspace_root}" || true)"
  fi
  if [[ -z "${version}" ]]; then
    echo "error: workspace.json#meiEnv.version is not set (refusing latest fallback)" >&2
    echo "hint: mei.sh env list --env ${mei_env_root} --tag ${tag}" >&2
    echo "hint: mei.sh env pin --workspace ${workspace_root} --env ${mei_env_root} --tag ${tag} --version <id>" >&2
    return 1
  fi
  if [[ "${version}" == "latest" ]]; then
    version="$(latest_version_under_tag "${mei_env_root}" "${tag}" || true)"
    if [[ -z "${version}" ]]; then
      echo "error: no usable bundle under ${mei_env_root}/targets/${tag}" >&2
      return 1
    fi
  fi
  bundle_root="${mei_env_root}/targets/${tag}/${version}"
  if [[ ! -d "${bundle_root}" && -d "${mei_env_root}/targets/${tag}/mei-lang-${version}" ]]; then
    bundle_root="${mei_env_root}/targets/${tag}/mei-lang-${version}"
    version="mei-lang-${version}"
  fi
  if [[ ! -d "${bundle_root}" ]]; then
    echo "error: mei-env bundle not found: ${bundle_root}" >&2
    echo "hint: mei.sh env list --env ${mei_env_root} --tag ${tag}" >&2
    return 1
  fi
  printf '%s' "${bundle_root}"
}

resolve_stock_deploy_dir() {
  local mei_lang_root
  mei_lang_root="$(resolve_mei_lang_root "$1")"
  local dir="${mei_lang_root}/stock/workspace/deploy"
  if [[ ! -d "${dir}" ]]; then
    echo "error: stock deploy templates missing at ${dir}" >&2
    return 1
  fi
  printf '%s' "${dir}"
}

resolve_workspace_root() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  cd "${script_dir}/.." && pwd
}

profile_target_subdir() {
  if [[ "${PROFILE}" == "release" ]]; then
    printf '%s' "release"
  else
    printf '%s' "debug"
  fi
}

sync_runtime_from_source() {
  # Legacy SOURCE/RUNTIME collapsed: always mei-env bins.
  SOURCE="mei-env"
  RUNTIME="mei-env"
  export SOURCE RUNTIME
}

apply_runtime_env_from_flags() {
  sync_runtime_from_source
  export PROFILE SOURCE RUNTIME
}

parse_common_args() {
  PROFILE="${MEI_PROFILE:-debug}"
  SOURCE="mei-env"
  RUNTIME="mei-env"
  DEPLOY_CONFIG_ARG="${MEI_WORKSPACE_CONFIG:-}"
  DEPLOY_LAUNCH_MODE="${MEI_LAUNCH:-}"
  DEPLOY_APP_CONFIGS=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --config) DEPLOY_CONFIG_ARG="$2"; shift 2 ;;
      --config=*) DEPLOY_CONFIG_ARG="${1#*=}"; shift ;;
      --launch) DEPLOY_LAUNCH_MODE="$2"; shift 2 ;;
      --launch=*) DEPLOY_LAUNCH_MODE="${1#*=}"; shift ;;
      --app-config)
        DEPLOY_APP_CONFIGS+=("$2")
        shift 2
        ;;
      --app-config=*)
        DEPLOY_APP_CONFIGS+=("${1#*=}")
        shift
        ;;
      --runtime) shift 2 ;; # ignored; mei-env only
      --runtime=*) shift ;;
      --cargo)
        echo "error: --cargo removed (0608); use mei.sh env build then build-app/start-host" >&2
        return 1
        ;;
      --force-build) export MEI_CARGO_FORCE_BUILD=1; shift ;;
      --release) PROFILE="release"; shift ;;
      --debug) PROFILE="debug"; shift ;;
      --profile) PROFILE="$2"; shift 2 ;;
      --profile=*) PROFILE="${1#*=}"; shift ;;
      *) break ;;
    esac
  done
  DEPLOY_CLI_ARGS=("$@")
  apply_runtime_env_from_flags
}

resolve_workspace_config_file() {
  local workspace_root="$1"
  local config_arg="${2:-}"
  local candidate=""

  if [[ -z "${config_arg}" ]]; then
    candidate="${workspace_root}/workspace.json"
    if [[ -f "${candidate}" ]]; then
      printf '%s' "${candidate}"
      return 0
    fi
    echo "error: workspace config not found: ${candidate}" >&2
    return 1
  fi

  if [[ "${config_arg}" == /* ]]; then
    candidate="${config_arg}"
  elif [[ -f "${workspace_root}/${config_arg}" ]]; then
    candidate="${workspace_root}/${config_arg}"
  elif [[ -f "${workspace_root}/configs/${config_arg}" ]]; then
    candidate="${workspace_root}/configs/${config_arg}"
  elif [[ -f "${workspace_root}/configs/${config_arg}.json" ]]; then
    candidate="${workspace_root}/configs/${config_arg}.json"
  else
    candidate="${workspace_root}/${config_arg}"
  fi

  if [[ ! -f "${candidate}" ]]; then
    echo "error: workspace config not found: ${candidate}" >&2
    echo "hint: use --config mini-park or --config configs/mini-park.json" >&2
    return 1
  fi
  printf '%s' "${candidate}"
}

default_app_from_workspace_config() {
  local config_file="$1"
  if ! command -v jq >/dev/null 2>&1; then
    return 0
  fi
  jq -r '.workspace.defaultApp // .deploy.accessEntry.defaultApp // empty' "${config_file}" 2>/dev/null || true
}

apply_workspace_deploy_env() {
  local workspace_root="$1"
  local config_arg="${DEPLOY_CONFIG_ARG:-${MEI_WORKSPACE_CONFIG:-}}"
  local config_file
  config_file="$(resolve_workspace_config_file "${workspace_root}" "${config_arg}")"
  export MEI_WORKSPACE_CONFIG="${config_file}"
  DEPLOY_WORKSPACE_CONFIG="${config_file}"
  local default_config="${workspace_root}/workspace.json"
  local derived
  derived="$(default_app_from_workspace_config "${config_file}")"
  if [[ -n "${derived}" ]]; then
    if [[ "${config_file}" != "${default_config}" || -z "${MEI_APP:-}" ]]; then
      export MEI_APP="${derived}"
    fi
  elif [[ -z "${MEI_APP:-}" ]]; then
    echo "error: MEI_APP unset and workspace config has no defaultApp; set MEI_APP or workspace.defaultApp" >&2
    return 1
  fi

  # deploy.devEval → MEI_DEV_EVAL_*（CLI/环境变量已显式设置时不覆盖）
  if command -v jq >/dev/null 2>&1 && [[ -f "${config_file}" ]]; then
    local dev_profile dev_eval_scopes dev_warmup_scopes
    dev_profile="$(jq -r '.deploy.devEval.profile // empty' "${config_file}" 2>/dev/null || true)"
    dev_eval_scopes="$(jq -r '(.deploy.devEval.evalScopes // .deploy.devEval.scopes // []) | join(",")' "${config_file}" 2>/dev/null || true)"
    dev_warmup_scopes="$(jq -r '(.deploy.devEval.warmupScopes // []) | join(",")' "${config_file}" 2>/dev/null || true)"
    if [[ -n "${dev_profile}" && -z "${MEI_DEV_EVAL_PROFILE:-}" ]]; then
      export MEI_DEV_EVAL_PROFILE="${dev_profile}"
    fi
    if [[ -n "${dev_eval_scopes}" && -z "${MEI_EVAL_SCOPE:-}" ]]; then
      export MEI_EVAL_SCOPE="${dev_eval_scopes}"
    fi
    if [[ -n "${dev_warmup_scopes}" && -z "${MEI_WARMUP_SCOPE:-}" ]]; then
      export MEI_WARMUP_SCOPE="${dev_warmup_scopes}"
    fi
  fi
}

runtime_json_path() {
  local workspace_root="$1"
  printf '%s' "${workspace_root}/deploy/runtime.json"
}

write_runtime_json() {
  local workspace_root="$1"
  local from="${2:-lang}"
  local mei_lang_root target_dir
  mei_lang_root="$(resolve_mei_lang_root "${workspace_root}")"
  target_dir="$(cargo_target_dir "${workspace_root}")"
  mkdir -p "${workspace_root}/deploy"
  cat >"$(runtime_json_path "${workspace_root}")" <<EOF
{
  "profile": "${PROFILE}",
  "from": "${from}",
  "meiLangRoot": "${mei_lang_root}",
  "targetDir": "${target_dir}",
  "installedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
}

read_runtime_json_field() {
  local workspace_root="$1"
  local field="$2"
  local path
  path="$(runtime_json_path "${workspace_root}")"
  if [[ ! -f "${path}" ]] || ! command -v jq >/dev/null 2>&1; then
    return 1
  fi
  jq -r ".${field} // empty" "${path}" 2>/dev/null || true
}

cargo_target_dir() {
  local workspace_root="$1"
  local mei_lang_root
  mei_lang_root="$(resolve_mei_lang_root "${workspace_root}")"
  printf '%s' "${MEI_CARGO_TARGET_DIR:-${mei_lang_root}/target}"
}

resolve_bin_path() {
  local workspace_root="$1"
  local bin_name="$2"
  local bundle_root src
  bundle_root="$(resolve_mei_env_bundle_root "${workspace_root}")" || return 1
  if ! src="$(find_release_binary "${bundle_root}" "${bin_name}")"; then
    echo "error: ${bin_name} not found under ${bundle_root}" >&2
    echo "hint: run mei.sh env build (expects …/v2/bin/${bin_name})" >&2
    return 1
  fi
  printf '%s' "${src}"
}

# Last WS-* generation line — legacy WS-YYYYMMDD.N or WS-YYYYMMDD-<git>[.N].
extract_ws_generation_id() {
  local raw="$1"
  local gen
  gen="$(printf '%s\n' "${raw}" | grep -E '^WS-[0-9]{8}(\.[0-9]+|-[0-9a-fA-F]{4,40}(\.[0-9]+)?)$' | tail -n 1 || true)"
  if [[ -z "${gen}" ]]; then
    return 1
  fi
  printf '%s' "${gen}"
}

# Ensure bins first (logs stay on the terminal), then capture only host-shell stdout.
capture_build_prepare_generation() {
  local workspace_root="$1"
  shift
  ensure_runtime_binaries "${workspace_root}"
  local raw gen host_shell
  host_shell="$(resolve_bin_path "${workspace_root}" "mei-host-shell")"
  raw="$("${host_shell}" "$@")"
  if ! gen="$(extract_ws_generation_id "${raw}")"; then
    echo "error: build prepare did not emit a WS-* generation id" >&2
    echo "captured output:" >&2
    printf '%s\n' "${raw}" >&2
    return 1
  fi
  printf '%s' "${gen}"
}

ensure_runtime_binaries() {
  local workspace_root="$1"
  local bin_name path
  for bin_name in mei-host-shell mei-compiler mei-app-runtime; do
    if ! path="$(resolve_bin_path "${workspace_root}" "${bin_name}")"; then
      return 1
    fi
    if [[ ! -x "${path}" ]]; then
      echo "error: not executable: ${path}" >&2
      echo "hint: run mei.sh env build then check workspace.json#meiEnv" >&2
      return 1
    fi
  done
  return 0
}

run_mei_plug_ds() {
  local workspace_root="$1"
  shift
  echo "error: standalone mei-plug-ds is retired; use mei-app-runtime (embedded DS)" >&2
  echo "error: workspace=${workspace_root} args=$*" >&2
  return 1
}

run_mei_app_runtime() {
  local workspace_root="$1"
  shift
  ensure_runtime_binaries "${workspace_root}"
  "$(resolve_bin_path "${workspace_root}" "mei-app-runtime")" "$@"
}

wait_for_plug_ds_health() {
  local host="${1:-127.0.0.1}"
  local port="${2:-9528}"
  local url="http://${host}:${port}/api/plug-ds/health"
  local attempt
  for attempt in $(seq 1 50); do
    if curl -sf "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.2
  done
  echo "error: plug-ds health check failed at ${url}" >&2
  return 1
}

run_mei_compiler() {
  local workspace_root="$1"
  shift
  ensure_runtime_binaries "${workspace_root}"
  "$(resolve_bin_path "${workspace_root}" "mei-compiler")" "$@"
}

run_mei_host_shell() {
  local workspace_root="$1"
  shift
  ensure_runtime_binaries "${workspace_root}"
  "$(resolve_bin_path "${workspace_root}" "mei-host-shell")" "$@"
}

discovered_app_ids() {
  local workspace_root="$1"
  local raw
  if ! raw="$(run_mei_host_shell "${workspace_root}" \
    apps list --workspace "${workspace_root}" --json 2>/dev/null)"; then
    return 1
  fi
  if ! command -v jq >/dev/null 2>&1; then
    echo "error: jq is required to parse discovered app ids" >&2
    return 1
  fi
  # apps list --json 返回对象数组：[{ "appId": "...", ... }, ...]
  jq -r '.[].appId // empty' <<<"${raw}"
}

ensure_build_generation_aligned() {
  local workspace_root="$1"
  shift
  local apps=("$@")
  if [[ ${#apps[@]} -eq 0 ]]; then
    echo "error: ensure_build_generation_aligned requires at least one app id (no business app default)" >&2
    return 1
  fi
  echo "==> align env generation (profile=${PROFILE}, source=${SOURCE}, runtime=${RUNTIME})"
  local prepare_args=(build prepare --workspace "${workspace_root}")
  local app_id
  for app_id in "${apps[@]}"; do
    prepare_args+=(--app "${app_id}")
  done
  MEI_ENV_GENERATION="$(capture_build_prepare_generation "${workspace_root}" "${prepare_args[@]}")"
  export MEI_ENV_GENERATION
  echo "envGeneration=${MEI_ENV_GENERATION}"
}

clean_retired_build_generations() {
  local workspace_root="$1"
  shift
  local apps=("$@")
  local clean_args=(build clean --workspace "${workspace_root}")
  local app_id
  for app_id in "${apps[@]}"; do
    clean_args+=(--app "${app_id}")
  done
  echo "==> clean retired env generations (retainBuildGenerations)"
  run_mei_host_shell "${workspace_root}" "${clean_args[@]}"
}

print_runtime_banner() {
  local workspace_root="$1"
  local host_shell="" mei_root tag ver ws_id
  mei_root="$(resolve_mei_env_root "${workspace_root}" 2>/dev/null || true)"
  tag="$(resolve_mei_env_target_tag "${workspace_root}" 2>/dev/null || true)"
  ver="$(read_workspace_mei_env_version "${workspace_root}" || true)"
  ws_id=""
  if command -v jq >/dev/null 2>&1 && [[ -f "${workspace_root}/workspace.json" ]]; then
    ws_id="$(jq -r '.workspace.id // .id // empty' "${workspace_root}/workspace.json" 2>/dev/null || true)"
  fi
  host_shell="$(resolve_bin_path "${workspace_root}" "mei-host-shell" 2>/dev/null || true)"
  echo "── runtime ──────────────────────────────────────"
  echo "Profile:     ${PROFILE:-debug}"
  echo "mei-env:     root=${mei_root:-?} tag=${tag:-?} version=${ver:-"(unset)"}"
  echo "workspace:   ${workspace_root}${ws_id:+ (id=${ws_id})}"
  echo "Binary:      ${host_shell:-"(unresolved)"}"
  echo "────────────────────────────────────────────────"
}

# Kill mei-app-runtime serve processes bound to this workspace only.
# Never use bare `pkill mei-app-runtime` (other workspaces may share the host).
sweep_stale_app_runtimes() {
  local workspace_root="$1"
  local workspace_abs
  workspace_abs="$(cd "${workspace_root}" && pwd -P)"
  local pids=()
  local pid cmd
  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    pid="$(awk '{print $1}' <<<"${line}")"
    cmd="$(sed -E 's/^[[:space:]]*[0-9]+[[:space:]]+//' <<<"${line}")"
    if [[ "${cmd}" == *mei-app-runtime* && "${cmd}" == *serve* && "${cmd}" == *"--workspace"* && "${cmd}" == *"${workspace_abs}"* ]]; then
      pids+=("${pid}")
    fi
  done < <(ps -axo pid=,command= 2>/dev/null || true)

  # Only signal PIDs that still exist (avoid "No such process" noise).
  local live=()
  for pid in "${pids[@]+"${pids[@]}"}"; do
    [[ -n "${pid}" ]] || continue
    if kill -0 "${pid}" 2>/dev/null; then
      live+=("${pid}")
    fi
  done
  if [[ "${#live[@]}" -eq 0 ]]; then
    return 0
  fi

  echo "sweeping ${#live[@]} stale mei-app-runtime for workspace=${workspace_abs}: ${live[*]}"
  for pid in "${live[@]}"; do
    kill -TERM "${pid}" 2>/dev/null || true
  done
  sleep 1
  for pid in "${live[@]}"; do
    if kill -0 "${pid}" 2>/dev/null; then
      kill -KILL "${pid}" 2>/dev/null || true
    fi
  done
}

# Report orphan / child mei-app-runtime counts for status.sh.
report_app_runtime_process_status() {
  local workspace_root="$1"
  local workspace_abs
  workspace_abs="$(cd "${workspace_root}" && pwd -P)"
  local host_pid=""
  if [[ -f "${workspace_root}/deploy/state/host.pid" ]]; then
    host_pid="$(cat "${workspace_root}/deploy/state/host.pid" 2>/dev/null || true)"
  fi

  local children=0
  local orphans=0
  local orphan_pids=()
  local pid ppid cmd
  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    pid="$(awk '{print $1}' <<<"${line}")"
    ppid="$(awk '{print $2}' <<<"${line}")"
    cmd="$(sed -E 's/^[[:space:]]*[0-9]+[[:space:]]+[0-9]+[[:space:]]+//' <<<"${line}")"
    if [[ "${cmd}" != *mei-app-runtime* || "${cmd}" != *serve* || "${cmd}" != *"--workspace"* || "${cmd}" != *"${workspace_abs}"* ]]; then
      continue
    fi
    if [[ -n "${host_pid}" && "${ppid}" == "${host_pid}" ]]; then
      children=$((children + 1))
    elif [[ "${ppid}" == "1" ]]; then
      orphans=$((orphans + 1))
      orphan_pids+=("${pid}")
    fi
  done < <(ps -axo pid=,ppid=,command= 2>/dev/null || true)

  echo "app-runtime.children=${children}"
  echo "app-runtime.orphans(ppid=1)=${orphans}"
  if [[ "${#orphan_pids[@]}" -gt 0 ]]; then
    echo "app-runtime.orphan_pids=${orphan_pids[*]}"
  fi

  local tiles_dir="${workspace_abs}/stock/gis/tiles"
  local martin_total=0
  local martin_orphans=0
  local martin_orphan_pids=()
  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    pid="$(awk '{print $1}' <<<"${line}")"
    ppid="$(awk '{print $2}' <<<"${line}")"
    cmd="$(sed -E 's/^[[:space:]]*[0-9]+[[:space:]]+[0-9]+[[:space:]]+//' <<<"${line}")"
    if [[ "${cmd}" != *martin* || "${cmd}" != *"--listen"* || "${cmd}" != *"${tiles_dir}"* ]]; then
      continue
    fi
    martin_total=$((martin_total + 1))
    if [[ "${ppid}" == "1" ]]; then
      martin_orphans=$((martin_orphans + 1))
      martin_orphan_pids+=("${pid}")
    fi
  done < <(ps -axo pid=,ppid=,command= 2>/dev/null || true)
  echo "martin.total=${martin_total}"
  echo "martin.orphans(ppid=1)=${martin_orphans}"
  if [[ "${#martin_orphan_pids[@]}" -gt 0 ]]; then
    echo "martin.orphan_pids=${martin_orphan_pids[*]}"
  fi
}

# Kill managed Martin processes serving this workspace's stock/gis/tiles.
# Host may leave orphans after kill -9 / IDE Stop; stop.sh must sweep them.
sweep_stale_managed_martin() {
  local workspace_root="$1"
  local workspace_abs tiles_dir
  workspace_abs="$(cd "${workspace_root}" && pwd -P)"
  tiles_dir="${workspace_abs}/stock/gis/tiles"
  local pids=()
  local pid cmd
  while IFS= read -r line; do
    [[ -n "${line}" ]] || continue
    pid="$(awk '{print $1}' <<<"${line}")"
    cmd="$(sed -E 's/^[[:space:]]*[0-9]+[[:space:]]+//' <<<"${line}")"
    if [[ "${cmd}" == *martin* && "${cmd}" == *"--listen"* && "${cmd}" == *"${tiles_dir}"* ]]; then
      pids+=("${pid}")
    fi
  done < <(ps -axo pid=,command= 2>/dev/null || true)

  local live=()
  for pid in "${pids[@]+"${pids[@]}"}"; do
    [[ -n "${pid}" ]] || continue
    if kill -0 "${pid}" 2>/dev/null; then
      live+=("${pid}")
    fi
  done
  if [[ "${#live[@]}" -eq 0 ]]; then
    return 0
  fi

  echo "sweeping ${#live[@]} stale martin for tiles=${tiles_dir}: ${live[*]}"
  for pid in "${live[@]}"; do
    kill -TERM "${pid}" 2>/dev/null || true
  done
  sleep 0.5
  for pid in "${live[@]}"; do
    if kill -0 "${pid}" 2>/dev/null; then
      kill -KILL "${pid}" 2>/dev/null || true
    fi
  done
}

run_workspace_serve() {
  local workspace_root="$1"
  shift
  local deploy_dir="${DEPLOY_DIR:?set DEPLOY_DIR before calling run_workspace_serve}"
  # Prevent stacking orphans from a previous hard-killed host.
  sweep_stale_app_runtimes "${workspace_root}"
  sweep_stale_managed_martin "${workspace_root}"
  local host port skip_prebuild prebuild_before_serve background warmup_policy auth_flag app
  # CLI --host/--port override these; see default_workspace_serve_* for MEI_* / workspace.json
  host="$(default_workspace_serve_host "${workspace_root}")"
  port="$(default_workspace_serve_port "${workspace_root}")"
  skip_prebuild="${MEI_SKIP_PREBUILD:-0}"
  prebuild_before_serve="${MEI_PREBUILD_BEFORE_SERVE:-0}"
  background="${MEI_SERVE_BACKGROUND:-0}"
  warmup_policy="${MEI_WARMUP_POLICY:-home}"
  auth_flag=""
  app="${MEI_APP:-}"
  if [[ "${MEI_AUTH:-0}" == "1" ]]; then
    auth_flag="--auth"
  fi

  DEPLOY_CONFIG_ARG="${MEI_WORKSPACE_CONFIG:-}"
  DEPLOY_LAUNCH="${MEI_LAUNCH:-${DEPLOY_LAUNCH:-0}}"
  DEPLOY_MODE="${MEI_MODE:-${DEPLOY_MODE:-}}"
  # Do NOT use ("${arr[@]:-}") — when unset it becomes a one-element empty array
  # and falsely trips "--launch and --app are mutually exclusive".
  DEPLOY_APP_CONFIGS=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --skip-prebuild) skip_prebuild=1; shift ;;
      --prebuild-first) prebuild_before_serve=1; shift ;;
      --background) background=1; shift ;;
      --auth) auth_flag="--auth"; shift ;;
      --app) app="$2"; shift 2 ;;
      --app=*) app="${1#*=}"; shift ;;
      --mode) DEPLOY_MODE="$2"; shift 2 ;;
      --mode=*) DEPLOY_MODE="${1#*=}"; shift ;;
      --config) DEPLOY_CONFIG_ARG="$2"; shift 2 ;;
      --config=*) DEPLOY_CONFIG_ARG="${1#*=}"; shift ;;
      --launch)
        if [[ $# -gt 1 && "$2" != -* ]]; then
          case "$2" in
            none|0|false) DEPLOY_LAUNCH=0 ;;
            all|1|true) DEPLOY_LAUNCH=1 ;;
            *)
              echo "error: unknown --launch value '$2' (use bare --launch, or all (or bare --launch))" >&2
              return 1
              ;;
          esac
          shift 2
        else
          DEPLOY_LAUNCH=1
          shift
        fi
        ;;
      --launch=*)
        case "${1#*=}" in
          none|0|false) DEPLOY_LAUNCH=0 ;;
          all|1|true|"") DEPLOY_LAUNCH=1 ;;
          *)
            echo "error: unknown --launch value '${1#*=}'" >&2
            return 1
            ;;
        esac
        shift
        ;;
      --app-config)
        DEPLOY_APP_CONFIGS+=("$2")
        shift 2
        ;;
      --app-config=*)
        DEPLOY_APP_CONFIGS+=("${1#*=}")
        shift
        ;;
      --port) port="$2"; shift 2 ;;
      --port=*) port="${1#*=}"; shift ;;
      --host) host="$2"; shift 2 ;;
      --host=*) host="${1#*=}"; shift ;;
      --policy) warmup_policy="$2"; shift 2 ;;
      --policy=*) warmup_policy="${1#*=}"; shift ;;
      --runtime) shift 2 ;;
      --runtime=*) shift ;;
      --cargo)
        echo "error: --cargo removed (0608); run mei.sh env build then start-host" >&2
        return 1
        ;;
      --force-build) export MEI_CARGO_FORCE_BUILD=1; shift ;;
      --release) PROFILE="release"; shift ;;
      --debug) PROFILE="debug"; shift ;;
      --profile) PROFILE="$2"; shift 2 ;;
      --profile=*) PROFILE="${1#*=}"; shift ;;
      *) break ;;
    esac
  done

  apply_runtime_env_from_flags
  export MEI_PROFILE="${PROFILE}" MEI_SOURCE="${SOURCE}" MEI_RUNTIME="${RUNTIME}"
  # Always export package root so /app-assets and /app-bundles resolve
  # (release bins / odd bake paths must not fall back to a missing CARGO_MANIFEST_DIR tree).
  local mei_lang_root
  if mei_lang_root="$(resolve_mei_lang_root "${workspace_root}" 2>/dev/null)"; then
    export MEI_LANG_ROOT="${MEI_LANG_ROOT:-${mei_lang_root}}"
    export MEI_PACKAGE_ROOT="${MEI_PACKAGE_ROOT:-${MEI_LANG_ROOT}}"
  else
    # installed / no sibling mei-lang: use mei-env bundle as package root when possible
    local bundle_root
    if bundle_root="$(resolve_mei_env_bundle_root "${workspace_root}" 2>/dev/null)"; then
      export MEI_PACKAGE_ROOT="${MEI_PACKAGE_ROOT:-${bundle_root}}"
      if [[ -d "${bundle_root}/v2/share/mei" ]]; then
        export MEI_LANG_ROOT="${MEI_LANG_ROOT:-${bundle_root}/v2/share/mei}"
      fi
    fi
  fi
  # GIS 默认：由 mei-host-shell 托管 Martin（stock/gis/tiles + 随机端口）。
  # Docker / 外部 Martin：MEI_GIS_USE_DOCKER_MARTIN=1 或自行设置 MEI_GIS_PROXY_UPSTREAM。
  if [[ "${MEI_GIS_USE_DOCKER_MARTIN:-0}" == "1" ]]; then
    export MEI_GIS_PROXY_UPSTREAM="${MEI_GIS_PROXY_UPSTREAM:-http://127.0.0.1:18080}"
    echo "==> GIS: Docker Martin（MEI_GIS_USE_DOCKER_MARTIN=1 → ${MEI_GIS_PROXY_UPSTREAM}）"
  elif [[ -n "${MEI_GIS_PROXY_UPSTREAM:-}" ]]; then
    echo "==> GIS: 外部上游 MEI_GIS_PROXY_UPSTREAM=${MEI_GIS_PROXY_UPSTREAM}"
  else
    unset MEI_GIS_PROXY_UPSTREAM || true
    echo "==> GIS: Host 托管 Martin（不设 MEI_GIS_PROXY_UPSTREAM）"
  fi

  if [[ "${DEPLOY_LAUNCH}" == "1" || ${#DEPLOY_APP_CONFIGS[@]} -gt 0 ]]; then
    if [[ -n "${app}" ]]; then
      echo "error: --launch and --app are mutually exclusive" >&2
      return 1
    fi
    unset MEI_APP
    app=""
  elif [[ -n "${app}" ]]; then
    export MEI_APP="${app}"
  else
    unset MEI_APP
  fi

  if [[ -n "${DEPLOY_MODE}" && -z "${app}" ]]; then
    echo "error: --mode requires --app <app_id>" >&2
    return 1
  fi

  ensure_runtime_binaries "${workspace_root}"

  local state_dir="${workspace_root}/deploy/state"
  mkdir -p "${state_dir}"

  if [[ -z "${app}" ]]; then
    prebuild_before_serve=0
    unset MEI_SERVE_EARLY_BIND MEI_DEFER_WARMUP_TO_PREBUILD
    echo "==> control-plane first boot — prebuild is deferred until profile apply"
  elif [[ "${skip_prebuild}" -eq 1 ]]; then
    prebuild_before_serve=0
  elif [[ "${prebuild_before_serve}" -eq 1 ]]; then
    export MEI_SERVE_EARLY_BIND=0
    unset MEI_DEFER_WARMUP_TO_PREBUILD
    echo "==> prebuild (policy=${warmup_policy}, app=${app}) — blocking before serve"
    local prebuild_args=()
    if [[ "${PROFILE}" == "release" ]]; then
      prebuild_args+=(--release)
    fi
    MEI_WARMUP_POLICY="${warmup_policy}" MEI_RUNTIME="${RUNTIME}" MEI_PROFILE="${PROFILE}" \
      MEI_APP="${app}" \
      MEI_WORKSPACE_CONFIG="${MEI_WORKSPACE_CONFIG:-}" \
      "${deploy_dir}/build-app.sh" "${prebuild_args[@]}"
    echo ""
  elif [[ -n "${app}" ]]; then
    echo "==> prebuild deferred — host binds first; warmup logs stream below"
    echo "    (also saved: deploy/state/prebuild.log)"
    export MEI_SERVE_EARLY_BIND=1
    export MEI_DEFER_WARMUP_TO_PREBUILD=1
    local prebuild_args=()
    if [[ "${PROFILE}" == "release" ]]; then
      prebuild_args+=(--release)
    fi
    (
      MEI_WARMUP_POLICY="${warmup_policy}" MEI_RUNTIME="${RUNTIME}" MEI_PROFILE="${PROFILE}" \
        MEI_APP="${app}" \
        MEI_WORKSPACE_CONFIG="${MEI_WORKSPACE_CONFIG:-}" \
        "${deploy_dir}/build-app.sh" "${prebuild_args[@]}" 2>&1 | tee -a "${state_dir}/prebuild.log"
    ) &
    echo $! >"${state_dir}/prebuild.pid"
  fi

  local url="http://${host}:${port}/runtime"
  if [[ -n "${app}" ]]; then
    url="http://${host}:${port}/apps/${app}/home"
  fi
  echo "Workspace: ${workspace_root}"
  if [[ -n "${DEPLOY_CONFIG_ARG:-}" ]]; then
    echo "Config:    ${DEPLOY_CONFIG_ARG}"
  fi
  if [[ "${DEPLOY_LAUNCH}" == "1" ]]; then
    echo "Launch:    all apps (launch.json defaultMode)"
  fi
  if [[ -n "${app}" ]]; then
    echo "App:       ${app}${DEPLOY_MODE:+ (mode=${DEPLOY_MODE})}"
  fi
  if [[ -n "${MEI_DEV_EVAL_PROFILE:-}" ]]; then
    echo "DevEval:   profile=${MEI_DEV_EVAL_PROFILE} eval=${MEI_EVAL_SCOPE:-} warmup=${MEI_WARMUP_SCOPE:-}"
  fi
  print_runtime_banner "${workspace_root}"
  if [[ -n "${MEI_PLUG_DS_URL:-}" ]]; then
    echo "Plug-ds:   external ${MEI_PLUG_DS_URL}"
  else
    echo "Plug-ds:   managed by host-shell (random local port)"
  fi
  echo "Listen:    http://${host}:${port}"
  echo "Open:      ${url}"
  echo ""

  local host_pid_file="${state_dir}/host.pid"
  local dev_eval_args=()
  local app_args=()
  local workspace_config_args=()
  local launch_args=()
  if [[ -n "${app}" ]]; then
    app_args+=(--app "${app}")
    if [[ -n "${DEPLOY_MODE}" ]]; then
      app_args+=(--mode "${DEPLOY_MODE}")
    fi
  fi
  if [[ -n "${DEPLOY_CONFIG_ARG:-}" ]]; then
    workspace_config_args+=(--workspace-config "${DEPLOY_CONFIG_ARG}")
  fi
  if [[ "${DEPLOY_LAUNCH}" == "1" ]]; then
    launch_args+=(--launch)
  fi
  for cfg in "${DEPLOY_APP_CONFIGS[@]:-}"; do
    [[ -n "${cfg}" ]] || continue
    launch_args+=(--app-config "${cfg}")
  done
  if [[ -n "${MEI_DEV_EVAL_PROFILE:-}" ]]; then
    dev_eval_args+=(--dev-eval-profile "${MEI_DEV_EVAL_PROFILE}")
  fi
  if [[ -n "${MEI_EVAL_SCOPE:-}" ]]; then
    dev_eval_args+=(--eval-scope "${MEI_EVAL_SCOPE}")
  fi
  if [[ -n "${MEI_WARMUP_SCOPE:-}" ]]; then
    dev_eval_args+=(--warmup-scope "${MEI_WARMUP_SCOPE}")
  fi

  if [[ "${background}" -eq 1 ]]; then
    # Avoid `printf '%q ' "${empty[@]}"` — on macOS/bash it emits a literal '' arg and clap fails.
    local q_app="" q_ws_cfg="" q_launch="" q_dev=""
    if ((${#app_args[@]})); then
      q_app="$(printf '%q ' "${app_args[@]}")"
    fi
    if ((${#workspace_config_args[@]})); then
      q_ws_cfg="$(printf '%q ' "${workspace_config_args[@]}")"
    fi
    if ((${#launch_args[@]})); then
      q_launch="$(printf '%q ' "${launch_args[@]}")"
    fi
    if ((${#dev_eval_args[@]})); then
      q_dev="$(printf '%q ' "${dev_eval_args[@]}")"
    fi
    nohup bash -c "
      source '${MEI_DEPLOY_LIB_PATH:?}'
      PROFILE='${PROFILE}'
      SOURCE='${SOURCE}'
      apply_runtime_env_from_flags
      export MEI_LANG_ROOT='${MEI_LANG_ROOT}'
      export MEI_PACKAGE_ROOT='${MEI_PACKAGE_ROOT}'
      export MEI_APP='${app}'
      export MEI_DEV_EVAL_PROFILE='${MEI_DEV_EVAL_PROFILE:-}'
      export MEI_EVAL_SCOPE='${MEI_EVAL_SCOPE:-}'
      export MEI_WARMUP_SCOPE='${MEI_WARMUP_SCOPE:-}'
      run_mei_host_shell '${workspace_root}' \
        serve --workspace '${workspace_root}' \
        ${q_app} ${q_ws_cfg} ${q_launch} \
        --host '${host}' --port '${port}' ${auth_flag} ${q_dev}
    " >"${state_dir}/host.log" 2>&1 &
    echo $! >"${host_pid_file}"
    echo "host-shell pid=$(cat "${host_pid_file}") log=deploy/state/host.log"
    return 0
  fi

  run_mei_host_shell "${workspace_root}" \
    serve --workspace "${workspace_root}" \
    ${app_args[@]+"${app_args[@]}"} \
    ${workspace_config_args[@]+"${workspace_config_args[@]}"} \
    ${launch_args[@]+"${launch_args[@]}"} \
    --host "${host}" --port "${port}" ${auth_flag} \
    ${dev_eval_args[@]+"${dev_eval_args[@]}"} "$@"
}


emit_deploy_status_banner() {
  local title="$1"
  local border_color="$2"
  local title_color="$3"
  shift 3
  local width=58
  local border
  border="$(printf '═%.0s' $(seq 1 "${width}"))"
  if [[ -t 1 ]]; then
    echo -e "\033[${border_color}m${border}\033[0m"
    echo -e "\033[${title_color}m  ✓ ${title}\033[0m"
    for line in "$@"; do
      echo "  ${line}"
    done
    echo -e "\033[${border_color}m${border}\033[0m"
  else
    echo "==> ${title}"
    for line in "$@"; do
      echo "    ${line}"
    done
  fi
}

emit_prebuild_pipeline_complete_banner() {
  local build_id="$1"
  shift
  local app_list
  app_list="$(printf '%s, ' "$@")"
  app_list="${app_list%, }"
  emit_deploy_status_banner \
    "编译流水线结束 · PREBUILD PIPELINE OK" \
    "1;33" "1;33;1" \
    "envVersion=${build_id} | apps=${app_list}" \
    "compile / import / plug-ds script finished" \
    "green ACCESS READY is emitted by host after every app is ready"
}
