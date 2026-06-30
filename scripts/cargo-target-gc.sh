#!/usr/bin/env bash
# Auto-run `cargo clean` when the Cargo target directory exceeds a size budget.
#
# Env:
#   MEI_CARGO_TARGET_GC=0           disable auto clean (default: 1)
#   MEI_CARGO_TARGET_MAX_GB=5       budget in GiB (default: 5)
#   MEI_CARGO_TARGET_MAX_BYTES=…    override budget in bytes
#   MEI_CARGO_TARGET_GC_DRY_RUN=1   print action only, do not clean
#   CARGO_TARGET_DIR                target directory (default: <mei-lang>/target)

set -euo pipefail

_cargo_target_gc_max_bytes() {
  if [[ -n "${MEI_CARGO_TARGET_MAX_BYTES:-}" ]]; then
    printf '%s' "${MEI_CARGO_TARGET_MAX_BYTES}"
    return 0
  fi
  local max_gb="${MEI_CARGO_TARGET_MAX_GB:-5}"
  if [[ ! "${max_gb}" =~ ^[0-9]+$ ]]; then
    echo "error: MEI_CARGO_TARGET_MAX_GB must be an integer, got: ${max_gb}" >&2
    return 1
  fi
  printf '%s' "$((max_gb * 1024 * 1024 * 1024))"
}

_cargo_target_gc_human_bytes() {
  local bytes="$1"
  if (( bytes >= 1073741824 )); then
    awk -v b="${bytes}" 'BEGIN { printf "%.1fGB", b / 1073741824 }'
  elif (( bytes >= 1048576 )); then
    awk -v b="${bytes}" 'BEGIN { printf "%.1fMB", b / 1048576 }'
  elif (( bytes >= 1024 )); then
    awk -v b="${bytes}" 'BEGIN { printf "%.1fKB", b / 1024 }'
  else
    printf '%dB' "${bytes}"
  fi
}

maybe_cargo_target_gc() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${CARGO_TARGET_DIR:-${mei_lang_root}/target}"

  if [[ "${MEI_CARGO_TARGET_GC:-1}" == "0" ]]; then
    return 0
  fi
  if [[ ! -d "${target_dir}" ]]; then
    return 0
  fi

  local max_bytes size_kb size_bytes
  max_bytes="$(_cargo_target_gc_max_bytes)"
  size_kb="$(du -sk "${target_dir}" 2>/dev/null | awk '{print $1}')"
  if [[ -z "${size_kb}" || ! "${size_kb}" =~ ^[0-9]+$ ]]; then
    echo "warn: cargo target GC skipped; could not measure ${target_dir}" >&2
    return 0
  fi
  size_bytes=$((size_kb * 1024))

  if (( size_bytes <= max_bytes )); then
    return 0
  fi

  local human_size human_max
  human_size="$(_cargo_target_gc_human_bytes "${size_bytes}")"
  human_max="$(_cargo_target_gc_human_bytes "${max_bytes}")"

  echo "==> cargo target GC: ${target_dir} (${human_size}) exceeds budget (${human_max})" >&2
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "==> cargo target GC: dry-run; would run cargo clean" >&2
    return 0
  fi

  CARGO_TARGET_DIR="${target_dir}" cargo clean --manifest-path "${mei_lang_root}/Cargo.toml" >&2
  echo "==> cargo target GC: cargo clean complete" >&2
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  mei_lang_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  maybe_cargo_target_gc "${mei_lang_root}"
fi
