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
      *) break ;;
    esac
  done
}

ensure_local_bins() {
  local workspace_root="$1"
  local bin_dir="${workspace_root}/deploy/bin"
  if [[ -x "${bin_dir}/mei-host-shell" && -x "${bin_dir}/mei-compiler" ]]; then
    return 0
  fi
  echo "==> local binaries missing; running install.sh"
  "${workspace_root}/deploy/install.sh"
}

run_mei_compiler() {
  local workspace_root="$1"
  shift
  if [[ "${RUNTIME}" == "cargo" ]]; then
    local mei_lang_root target_dir
    mei_lang_root="$(resolve_mei_lang_root "${workspace_root}")"
    target_dir="${CARGO_TARGET_DIR:-${mei_lang_root}/target}"
    CARGO_TARGET_DIR="${target_dir}" cargo run --manifest-path "${mei_lang_root}/Cargo.toml" \
      -p mei-compiler -- "$@"
    return
  fi
  ensure_local_bins "${workspace_root}"
  "${workspace_root}/deploy/bin/mei-compiler" "$@"
}

run_mei_host_shell() {
  local workspace_root="$1"
  shift
  if [[ "${RUNTIME}" == "cargo" ]]; then
    local mei_lang_root target_dir
    mei_lang_root="$(resolve_mei_lang_root "${workspace_root}")"
    target_dir="${CARGO_TARGET_DIR:-${mei_lang_root}/target}"
    CARGO_TARGET_DIR="${target_dir}" cargo run --manifest-path "${mei_lang_root}/Cargo.toml" \
      -p mei-host-shell -- "$@"
    return
  fi
  ensure_local_bins "${workspace_root}"
  "${workspace_root}/deploy/bin/mei-host-shell" "$@"
}
