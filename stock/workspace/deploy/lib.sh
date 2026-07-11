#!/usr/bin/env bash
# Workspace deploy runtime: PROFILE (debug|release) + SOURCE (installed|lang).
set -euo pipefail

PROFILE="${MEI_PROFILE:-debug}"
SOURCE="${MEI_SOURCE:-installed}"
RUNTIME="${MEI_RUNTIME:-local}"

resolve_mei_lang_root() {
  local workspace_root="$1"
  local root="${MEI_LANG_ROOT:-${workspace_root}/../../mei-lang}"
  if [[ ! -f "${root}/Cargo.toml" ]]; then
    echo "error: mei-lang not found at ${root} (set MEI_LANG_ROOT)" >&2
    return 1
  fi
  printf '%s' "${root}"
}

resolve_mei_release_root() {
  local workspace_root="$1"
  local root="${MEI_RELEASE_ROOT:-${workspace_root}/../../mei-release}"
  if [[ ! -d "${root}" ]]; then
    echo "error: mei-release not found at ${root} (set MEI_RELEASE_ROOT)" >&2
    return 1
  fi
  printf '%s' "${root}"
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
  case "${SOURCE}" in
    lang)
      RUNTIME="cargo"
      ;;
    installed|release)
      RUNTIME="local"
      ;;
    *)
      echo "error: unknown SOURCE=${SOURCE}" >&2
      return 1
      ;;
  esac
  export RUNTIME
}

apply_runtime_env_from_flags() {
  if [[ "${RUNTIME}" == "cargo" ]]; then
    SOURCE="lang"
  fi
  sync_runtime_from_source
  export PROFILE SOURCE RUNTIME
}

parse_common_args() {
  PROFILE="${MEI_PROFILE:-debug}"
  SOURCE="${MEI_SOURCE:-installed}"
  RUNTIME="${MEI_RUNTIME:-local}"
  DEPLOY_CONFIG_ARG="${MEI_WORKSPACE_CONFIG:-}"
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --config) DEPLOY_CONFIG_ARG="$2"; shift 2 ;;
      --config=*) DEPLOY_CONFIG_ARG="${1#*=}"; shift ;;
      --runtime) RUNTIME="$2"; shift 2 ;;
      --runtime=*) RUNTIME="${1#*=}"; shift ;;
      --cargo) SOURCE="lang"; shift ;;
      --force-build) export MEI_CARGO_FORCE_BUILD=1; shift ;;
      --release) PROFILE="release"; shift ;;
      --debug) PROFILE="debug"; shift ;;
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
    export MEI_APP="data-demo"
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
  printf '%s' "${CARGO_TARGET_DIR:-${mei_lang_root}/target}"
}

resolve_bin_path() {
  local workspace_root="$1"
  local bin_name="$2"
  if [[ "${SOURCE}" == "lang" ]]; then
    printf '%s/%s/%s' "$(cargo_target_dir "${workspace_root}")" "$(profile_target_subdir)" "${bin_name}"
    return 0
  fi
  printf '%s' "${workspace_root}/deploy/bin/${bin_name}"
}

ensure_local_bins() {
  local workspace_root="$1"
  local bin_dir="${workspace_root}/deploy/bin"
  if [[ -x "${bin_dir}/mei-host-shell" && -x "${bin_dir}/mei-compiler" && -x "${bin_dir}/mei-plug-ds" ]]; then
    return 0
  fi
  echo "==> local binaries missing; running install.sh"
  "${workspace_root}/deploy/install.sh"
}

cargo_runtime_bins_ready() {
  local workspace_root="$1"
  local bin_name
  for bin_name in mei-host-shell mei-compiler mei-plug-ds; do
    if [[ ! -x "$(resolve_bin_path "${workspace_root}" "${bin_name}")" ]]; then
      return 1
    fi
  done
  return 0
}

run_cargo_runtime_build() {
  local workspace_root="$1"
  local mei_lang_root target_dir build_script
  mei_lang_root="$(resolve_mei_lang_root "${workspace_root}")"
  target_dir="$(cargo_target_dir "${workspace_root}")"
  build_script="${mei_lang_root}/scripts/build.sh"
  export MEI_CARGO_BUILD_PROFILE="${PROFILE}"

  if [[ -f "${build_script}" ]]; then
    if [[ "${PROFILE}" == "release" ]]; then
      MEI_CARGO_TARGET_HYGIENE_RAN="${MEI_CARGO_TARGET_HYGIENE_RAN:-0}" \
        MEI_CARGO_RUNTIME_PANEL_EMITTED="${MEI_CARGO_RUNTIME_PANEL_EMITTED:-0}" \
        CARGO_TARGET_DIR="${target_dir}" "${build_script}" --release
    else
      MEI_CARGO_TARGET_HYGIENE_RAN="${MEI_CARGO_TARGET_HYGIENE_RAN:-0}" \
        MEI_CARGO_RUNTIME_PANEL_EMITTED="${MEI_CARGO_RUNTIME_PANEL_EMITTED:-0}" \
        CARGO_TARGET_DIR="${target_dir}" "${build_script}" --debug
    fi
    return 0
  fi

  if [[ "${MEI_CARGO_TARGET_HYGIENE:-1}" != "0" && "${MEI_CARGO_TARGET_HYGIENE_RAN:-0}" != "1" ]]; then
    # shellcheck source=/dev/null
    source "${mei_lang_root}/scripts/cargo-target-gc.sh"
    maybe_cargo_target_hygiene "${mei_lang_root}"
  fi
  local cargo_args=(build --manifest-path "${mei_lang_root}/Cargo.toml" \
    -p mei-compiler -p mei-plug-ds -p mei-host-shell)
  if [[ "${PROFILE}" == "release" ]]; then
    cargo_args=(build --release --manifest-path "${mei_lang_root}/Cargo.toml" \
      -p mei-compiler -p mei-plug-ds -p mei-host-shell)
  fi
  CARGO_TARGET_DIR="${target_dir}" cargo "${cargo_args[@]}"
}

ensure_runtime_binaries() {
  local workspace_root="$1"
  if [[ "${SOURCE}" != "lang" ]]; then
    ensure_local_bins "${workspace_root}"
    return 0
  fi
  if [[ "${MEI_CARGO_RUNTIME_READY:-0}" == "1" ]]; then
    return 0
  fi

  local mei_lang_root target_dir gc_script build_plan
  mei_lang_root="$(resolve_mei_lang_root "${workspace_root}")"
  target_dir="$(cargo_target_dir "${workspace_root}")"
  gc_script="${mei_lang_root}/scripts/cargo-target-gc.sh"
  if [[ -f "${gc_script}" ]]; then
    # shellcheck source=/dev/null
    source "${gc_script}"
  fi

  unset MEI_CARGO_TARGET_HYGIENE_SUMMARY
  export MEI_CARGO_BUILD_PROFILE="${PROFILE}"
  if cargo_runtime_bins_ready "${workspace_root}"; then
    export MEI_CARGO_TARGET_DEFER_CLEAN=1
  fi
  if [[ "${MEI_CARGO_TARGET_HYGIENE:-1}" != "0" ]]; then
    maybe_cargo_target_hygiene "${mei_lang_root}"
    export MEI_CARGO_TARGET_HYGIENE_RAN=1
  fi

  build_plan="compile"
  if [[ "${MEI_CARGO_FORCE_BUILD:-0}" == "1" ]]; then
    build_plan="force-clean"
  elif [[ "${MEI_CARGO_SKIP_BUILD_IF_FRESH:-0}" == "1" ]]; then
    if cargo_runtime_bins_ready "${workspace_root}"; then
      build_plan="skip"
    fi
  fi

  if declare -F cargo_target_emit_startup_panel >/dev/null 2>&1; then
    cargo_target_emit_startup_panel "${target_dir}" "${PROFILE}" "${build_plan}" "" "${workspace_root}"
    export MEI_CARGO_RUNTIME_PANEL_EMITTED=1
  fi

  if [[ "${build_plan}" == "skip" ]]; then
    export MEI_CARGO_RUNTIME_READY=1
    return 0
  fi

  if [[ "${build_plan}" == "force-clean" ]]; then
    echo "==> force rebuild: cargo clean (profile=${PROFILE}, target=${target_dir})" >&2
    CARGO_TARGET_DIR="${target_dir}" cargo clean --manifest-path "${mei_lang_root}/Cargo.toml" >&2
  fi

  echo "==> building runtime binaries (profile=${PROFILE}, source=lang, mei-lang=${mei_lang_root})" >&2
  run_cargo_runtime_build "${workspace_root}"
  export MEI_CARGO_RUNTIME_READY=1
}

ensure_cargo_runtime_binaries() {
  SOURCE="lang"
  apply_runtime_env_from_flags
  ensure_runtime_binaries "$1"
}

run_mei_plug_ds() {
  local workspace_root="$1"
  shift
  ensure_runtime_binaries "${workspace_root}"
  "$(resolve_bin_path "${workspace_root}" "mei-plug-ds")" "$@"
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
  jq -r '.[]' <<<"${raw}"
}

ensure_build_generation_aligned() {
  local workspace_root="$1"
  local app="${2:-data-demo}"
  echo "==> align env generation (profile=${PROFILE}, source=${SOURCE}, runtime=${RUNTIME})"
  MEI_ENV_GENERATION="$(run_mei_host_shell "${workspace_root}" \
    build prepare --workspace "${workspace_root}" --app "${app}")"
  export MEI_ENV_GENERATION
  echo "envGeneration=${MEI_ENV_GENERATION}"
}

print_runtime_banner() {
  local workspace_root="$1"
  local host_shell
  host_shell="$(resolve_bin_path "${workspace_root}" "mei-host-shell")"
  echo "Profile:   ${PROFILE}"
  echo "Source:    ${SOURCE}"
  echo "Runtime:   ${RUNTIME} (impl)"
  echo "Binary:    ${host_shell}"
}

run_workspace_serve() {
  local workspace_root="$1"
  shift
  local deploy_dir="${DEPLOY_DIR:?set DEPLOY_DIR before calling run_workspace_serve}"
  local host port skip_prebuild prebuild_before_serve background warmup_policy auth_flag app
  host="${MEI_SERVE_HOST:-127.0.0.1}"
  port="${MEI_PORT:-9527}"
  skip_prebuild="${MEI_SKIP_PREBUILD:-0}"
  prebuild_before_serve="${MEI_PREBUILD_BEFORE_SERVE:-0}"
  background="${MEI_SERVE_BACKGROUND:-0}"
  warmup_policy="${MEI_WARMUP_POLICY:-home}"
  auth_flag=""
  if [[ "${MEI_AUTH:-0}" == "1" ]]; then
    auth_flag="--auth"
  fi

  DEPLOY_CONFIG_ARG="${MEI_WORKSPACE_CONFIG:-}"
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --skip-prebuild) skip_prebuild=1; shift ;;
      --prebuild-first) prebuild_before_serve=1; shift ;;
      --background) background=1; shift ;;
      --auth) auth_flag="--auth"; shift ;;
      --config) DEPLOY_CONFIG_ARG="$2"; shift 2 ;;
      --config=*) DEPLOY_CONFIG_ARG="${1#*=}"; shift ;;
      --port) port="$2"; shift 2 ;;
      --port=*) port="${1#*=}"; shift ;;
      --host) host="$2"; shift 2 ;;
      --host=*) host="${1#*=}"; shift ;;
      --policy) warmup_policy="$2"; shift 2 ;;
      --policy=*) warmup_policy="${1#*=}"; shift ;;
      --runtime) RUNTIME="$2"; shift 2 ;;
      --runtime=*) RUNTIME="${1#*=}"; shift ;;
      --cargo) SOURCE="lang"; shift ;;
      --force-build) export MEI_CARGO_FORCE_BUILD=1; shift ;;
      --release) PROFILE="release"; shift ;;
      --debug) PROFILE="debug"; shift ;;
      *) break ;;
    esac
  done

  apply_runtime_env_from_flags
  export MEI_PROFILE="${PROFILE}" MEI_SOURCE="${SOURCE}" MEI_RUNTIME="${RUNTIME}"

  if declare -F apply_workspace_deploy_env >/dev/null 2>&1; then
    apply_workspace_deploy_env "${workspace_root}"
  fi
  app="${MEI_APP:-data-demo}"
  export MEI_APP

  ensure_runtime_binaries "${workspace_root}"

  local state_dir="${workspace_root}/deploy/state"
  mkdir -p "${state_dir}"

  if [[ "${skip_prebuild}" -eq 1 ]]; then
    prebuild_before_serve=0
  elif [[ "${prebuild_before_serve}" -eq 1 ]]; then
    export MEI_SERVE_EARLY_BIND=0
    unset MEI_DEFER_WARMUP_TO_PREBUILD
    echo "==> prebuild (policy=${warmup_policy}, app=${app}) — blocking before serve"
    local prebuild_args=(--runtime "${RUNTIME}")
    if [[ "${PROFILE}" == "release" ]]; then
      prebuild_args+=(--release)
    fi
    MEI_WARMUP_POLICY="${warmup_policy}" MEI_RUNTIME="${RUNTIME}" MEI_PROFILE="${PROFILE}" \
      MEI_SOURCE="${SOURCE}" MEI_APP="${app}" \
      MEI_WORKSPACE_CONFIG="${MEI_WORKSPACE_CONFIG:-}" \
      "${deploy_dir}/prebuild.sh" "${prebuild_args[@]}"
    echo ""
  else
    echo "==> prebuild deferred — host binds first; warmup logs stream below"
    echo "    (also saved: deploy/state/prebuild.log)"
    export MEI_SERVE_EARLY_BIND=1
    export MEI_DEFER_WARMUP_TO_PREBUILD=1
    local prebuild_args=(--runtime "${RUNTIME}")
    if [[ "${PROFILE}" == "release" ]]; then
      prebuild_args+=(--release)
    fi
    (
      MEI_WARMUP_POLICY="${warmup_policy}" MEI_RUNTIME="${RUNTIME}" MEI_PROFILE="${PROFILE}" \
        MEI_SOURCE="${SOURCE}" MEI_APP="${app}" \
        MEI_WORKSPACE_CONFIG="${MEI_WORKSPACE_CONFIG:-}" \
        "${deploy_dir}/prebuild.sh" "${prebuild_args[@]}" 2>&1 | tee -a "${state_dir}/prebuild.log"
    ) &
    echo $! >"${state_dir}/prebuild.pid"
  fi

  local url="http://${host}:${port}/apps/app/${app}/scene/home"
  echo "Workspace: ${workspace_root}"
  if [[ -n "${MEI_WORKSPACE_CONFIG:-}" ]]; then
    echo "Config:    ${MEI_WORKSPACE_CONFIG}"
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

  if [[ "${background}" -eq 1 ]]; then
    nohup bash -c "
      source '${deploy_dir}/lib.sh'
      PROFILE='${PROFILE}'
      SOURCE='${SOURCE}'
      apply_runtime_env_from_flags
      export MEI_WORKSPACE_CONFIG='${MEI_WORKSPACE_CONFIG:-}'
      export MEI_APP='${app}'
      run_mei_host_shell '${workspace_root}' \
        serve --workspace '${workspace_root}' --app '${app}' \
        --host '${host}' --port '${port}' ${auth_flag} $*
    " >"${state_dir}/host.log" 2>&1 &
    echo $! >"${host_pid_file}"
    echo "host-shell pid=$(cat "${host_pid_file}") log=deploy/state/host.log"
    return 0
  fi

  run_mei_host_shell "${workspace_root}" \
    serve --workspace "${workspace_root}" --app "${app}" \
    --host "${host}" --port "${port}" ${auth_flag} "$@"
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
