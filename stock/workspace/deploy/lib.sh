#!/usr/bin/env bash
# Shared runtime resolution: local (deploy/bin) or cargo (mei-lang source).
set -euo pipefail

resolve_mei_lang_root() {
  local workspace_root="$1"
  local root="${MEI_LANG_ROOT:-${workspace_root}/../../mei-lang}"
  if [[ ! -f "${root}/Cargo.toml" ]]; then
    echo "error: mei-lang not found at ${root} (set MEI_LANG_ROOT)" >&2
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

parse_common_args() {
  RUNTIME="${MEI_RUNTIME:-local}"
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --runtime) RUNTIME="$2"; shift 2 ;;
      --runtime=*) RUNTIME="${1#*=}"; shift ;;
      --cargo) RUNTIME="cargo"; shift ;;
      *) break ;;
    esac
  done
  # Caller $@ is unchanged after a function call; stash extras for forwarding.
  DEPLOY_CLI_ARGS=("$@")
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

cargo_target_dir() {
  local workspace_root="$1"
  local mei_lang_root
  mei_lang_root="$(resolve_mei_lang_root "${workspace_root}")"
  printf '%s' "${CARGO_TARGET_DIR:-${mei_lang_root}/target}"
}

cargo_runtime_bin_path() {
  local workspace_root="$1"
  local bin_name="$2"
  printf '%s/debug/%s' "$(cargo_target_dir "${workspace_root}")" "${bin_name}"
}

ensure_cargo_runtime_binaries() {
  local workspace_root="$1"
  if [[ "${RUNTIME}" != "cargo" ]]; then
    return 0
  fi
  if [[ "${MEI_CARGO_RUNTIME_READY:-0}" == "1" ]]; then
    return 0
  fi
  local mei_lang_root target_dir
  mei_lang_root="$(resolve_mei_lang_root "${workspace_root}")"
  target_dir="$(cargo_target_dir "${workspace_root}")"
  echo "==> building cargo runtime binaries (mei-lang=${mei_lang_root})" >&2
  CARGO_TARGET_DIR="${target_dir}" cargo build --manifest-path "${mei_lang_root}/Cargo.toml" \
    -p mei-compiler -p mei-plug-ds -p mei-host-shell
  export MEI_CARGO_RUNTIME_READY=1
}

run_mei_plug_ds() {
  local workspace_root="$1"
  shift
  if [[ "${RUNTIME}" == "cargo" ]]; then
    ensure_cargo_runtime_binaries "${workspace_root}"
    "$(cargo_runtime_bin_path "${workspace_root}" "mei-plug-ds")" "$@"
    return
  fi
  ensure_local_bins "${workspace_root}"
  "${workspace_root}/deploy/bin/mei-plug-ds" "$@"
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
  if [[ "${RUNTIME}" == "cargo" ]]; then
    ensure_cargo_runtime_binaries "${workspace_root}"
    "$(cargo_runtime_bin_path "${workspace_root}" "mei-compiler")" "$@"
    return
  fi
  ensure_local_bins "${workspace_root}"
  "${workspace_root}/deploy/bin/mei-compiler" "$@"
}

run_mei_host_shell() {
  local workspace_root="$1"
  shift
  if [[ "${RUNTIME}" == "cargo" ]]; then
    ensure_cargo_runtime_binaries "${workspace_root}"
    "$(cargo_runtime_bin_path "${workspace_root}" "mei-host-shell")" "$@"
    return
  fi
  ensure_local_bins "${workspace_root}"
  "${workspace_root}/deploy/bin/mei-host-shell" "$@"
}

# Point build/active at env/{mei-lang-version}-ws{workspace.version} before compile/import.
ensure_build_generation_aligned() {
  local workspace_root="$1"
  local app="${2:-data-demo}"
  echo "==> align env generation with mei-lang CLI (runtime=${RUNTIME})"
  MEI_ENV_GENERATION="$(run_mei_host_shell "${workspace_root}" \
    build prepare --workspace "${workspace_root}" --app "${app}")"
  export MEI_ENV_GENERATION
  echo "envGeneration=${MEI_ENV_GENERATION}"
}
