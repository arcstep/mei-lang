#!/usr/bin/env bash
# Cargo target hygiene: sweep stale artifacts, then `cargo clean` when over budget.
#
# Env:
#   MEI_CARGO_TARGET_GC=0           disable auto clean (default: 1)
#   MEI_CARGO_TARGET_SWEEP=0          disable stale-artifact sweep (default: 1)
#   MEI_CARGO_TARGET_MAX_GB=5         budget in GiB (default: 5)
#   MEI_CARGO_TARGET_MAX_BYTES=…      override budget in bytes
#   MEI_CARGO_TARGET_GC_DRY_RUN=1     print action only, do not clean
#   MEI_CARGO_SWEEP_KEEP_PKGS=…       runtime closure roots (default: mei-compiler,mei-plug-ds,mei-host-shell)
#   MEI_CARGO_BUILD_PROFILE=debug|release  active build profile (default: debug)
#   CARGO_TARGET_DIR                  target directory (default: <mei-lang>/target)

set -euo pipefail

_cargo_target_gc_script_dir() {
  cd "$(dirname "${BASH_SOURCE[0]}")" && pwd
}

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

_cargo_target_dir_size_bytes() {
  local target_dir="$1"
  local size_kb
  size_kb="$(du -sk "${target_dir}" 2>/dev/null | awk '{print $1}')"
  if [[ -z "${size_kb}" || ! "${size_kb}" =~ ^[0-9]+$ ]]; then
    return 1
  fi
  printf '%s' "$((size_kb * 1024))"
}

_cargo_target_sweep_stale_bytes() {
  local target_dir="$1"
  local mei_lang_root="$2"
  local phase="${3:-stale}"
  local scripts_dir sweep_py dry_run_args keep_pkgs active_profile

  scripts_dir="$(_cargo_target_gc_script_dir)"
  sweep_py="${scripts_dir}/cargo-target-sweep-stale.py"
  if [[ ! -f "${sweep_py}" ]]; then
    echo "warn: cargo target sweep skipped; missing ${sweep_py}" >&2
    printf '%s' "missing"
    return 0
  fi

  dry_run_args=()
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    dry_run_args+=(--dry-run)
  fi

  keep_pkgs="${MEI_CARGO_SWEEP_KEEP_PKGS:-mei-compiler,mei-plug-ds,mei-host-shell}"
  active_profile="${MEI_CARGO_BUILD_PROFILE:-debug}"
  if [[ "${active_profile}" != "debug" && "${active_profile}" != "release" ]]; then
    active_profile="debug"
  fi

  if [[ "${phase}" == "incremental" ]]; then
    python3 "${sweep_py}" "${target_dir}" "${dry_run_args[@]}" \
      --incremental-only \
      2>/dev/null | awk -F= '/^freed_bytes=/{print $2; exit}'
    return 0
  fi

  python3 "${sweep_py}" "${target_dir}" "${dry_run_args[@]}" \
    --manifest-path "${mei_lang_root}/Cargo.toml" \
    --keep-packages "${keep_pkgs}" \
    --active-profile "${active_profile}" \
    --drop-inactive-profile \
    --sweep-tests \
    2>/dev/null | awk -F= '/^freed_bytes=/{print $2; exit}'
}

maybe_cargo_target_sweep_stale() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${2:?target_dir required}"
  local before_bytes="$3"
  local freed_bytes human before_human after_bytes after_human

  if [[ "${MEI_CARGO_TARGET_SWEEP:-1}" == "0" ]]; then
    echo "    sweep: disabled (MEI_CARGO_TARGET_SWEEP=0)" >&2
    printf '%s' "${before_bytes}"
    return 0
  fi
  if [[ ! -d "${target_dir}" ]]; then
    echo "    sweep: skipped (target directory missing)" >&2
    printf '%s' "${before_bytes}"
    return 0
  fi

  before_human="$(_cargo_target_gc_human_bytes "${before_bytes}")"
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "    sweep: dry-run stale cleanup (current ${before_human})" >&2
  else
    echo "    sweep: removing inactive profile, out-of-scope workspace, tests, stale deps (current ${before_human})" >&2
  fi

  freed_bytes="$(_cargo_target_sweep_stale_bytes "${target_dir}" "${mei_lang_root}" stale)"
  if [[ "${freed_bytes}" == "missing" ]]; then
    printf '%s' "${before_bytes}"
    return 0
  fi
  if [[ -z "${freed_bytes}" || ! "${freed_bytes}" =~ ^[0-9]+$ ]]; then
    echo "    sweep: skipped (could not measure reclaimed bytes)" >&2
    printf '%s' "${before_bytes}"
    return 0
  fi

  human="$(_cargo_target_gc_human_bytes "${freed_bytes}")"
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    after_bytes=$((before_bytes - freed_bytes))
    if (( after_bytes < 0 )); then
      after_bytes=0
    fi
  elif ! after_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
    after_bytes="$((before_bytes - freed_bytes))"
  fi
  after_human="$(_cargo_target_gc_human_bytes "${after_bytes}")"

  if (( freed_bytes <= 0 )); then
    echo "    sweep: no stale artifacts (${after_human}, unchanged)" >&2
    printf '%s' "${after_bytes}"
    return 0
  fi

  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "    sweep: would reclaim ${human} (${before_human} -> ${after_human})" >&2
  else
    echo "    sweep: reclaimed ${human} (${before_human} -> ${after_human})" >&2
  fi
  printf '%s' "${after_bytes}"
}

maybe_cargo_target_sweep_incremental() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${2:?target_dir required}"
  local before_bytes="$3"
  local freed_bytes human before_human after_bytes after_human

  if [[ "${MEI_CARGO_TARGET_SWEEP:-1}" == "0" ]]; then
    printf '%s' "${before_bytes}"
    return 0
  fi
  if [[ ! -d "${target_dir}" ]]; then
    printf '%s' "${before_bytes}"
    return 0
  fi

  before_human="$(_cargo_target_gc_human_bytes "${before_bytes}")"
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "    sweep(incremental): dry-run (current ${before_human})" >&2
  else
    echo "    sweep(incremental): still over budget; clearing incremental cache (current ${before_human})" >&2
  fi

  freed_bytes="$(_cargo_target_sweep_stale_bytes "${target_dir}" "${mei_lang_root}" incremental)"
  if [[ "${freed_bytes}" == "missing" ]]; then
    printf '%s' "${before_bytes}"
    return 0
  fi
  if [[ -z "${freed_bytes}" || ! "${freed_bytes}" =~ ^[0-9]+$ ]]; then
    echo "    sweep(incremental): skipped (could not measure reclaimed bytes)" >&2
    printf '%s' "${before_bytes}"
    return 0
  fi

  human="$(_cargo_target_gc_human_bytes "${freed_bytes}")"
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    after_bytes=$((before_bytes - freed_bytes))
    if (( after_bytes < 0 )); then
      after_bytes=0
    fi
  elif ! after_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
    after_bytes="$((before_bytes - freed_bytes))"
  fi
  after_human="$(_cargo_target_gc_human_bytes "${after_bytes}")"

  if (( freed_bytes <= 0 )); then
    echo "    sweep(incremental): no incremental cache (${after_human}, unchanged)" >&2
    printf '%s' "${after_bytes}"
    return 0
  fi

  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "    sweep(incremental): would reclaim ${human} (${before_human} -> ${after_human})" >&2
  else
    echo "    sweep(incremental): reclaimed ${human} (${before_human} -> ${after_human})" >&2
  fi
  printf '%s' "${after_bytes}"
}

maybe_cargo_target_gc() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${2:?target_dir required}"
  local max_bytes="$3"
  local size_bytes="${4:?size_bytes required}"
  local human_size human_max

  if [[ "${MEI_CARGO_TARGET_GC:-1}" == "0" ]]; then
    echo "    clean: disabled (MEI_CARGO_TARGET_GC=0)" >&2
    return 0
  fi
  if [[ ! -d "${target_dir}" ]]; then
    echo "    clean: skipped (target directory missing)" >&2
    return 0
  fi

  human_size="$(_cargo_target_gc_human_bytes "${size_bytes}")"
  human_max="$(_cargo_target_gc_human_bytes "${max_bytes}")"

  if (( size_bytes <= max_bytes )); then
    echo "    clean: not needed (${human_size} <= budget ${human_max})" >&2
    return 0
  fi

  echo "    clean: ${human_size} exceeds budget ${human_max}" >&2
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "    clean: dry-run; would run cargo clean" >&2
    return 0
  fi

  CARGO_TARGET_DIR="${target_dir}" cargo clean --manifest-path "${mei_lang_root}/Cargo.toml" >&2
  if size_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
    human_size="$(_cargo_target_gc_human_bytes "${size_bytes}")"
    echo "    clean: cargo clean complete (now ${human_size})" >&2
  else
    echo "    clean: cargo clean complete" >&2
  fi
}

# Run before cargo build: sweep stale artifacts, then enforce the size budget.
maybe_cargo_target_hygiene() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${CARGO_TARGET_DIR:-${mei_lang_root}/target}"
  local max_bytes before_bytes human_before human_max sweep_on clean_on after_sweep_bytes

  if [[ "${MEI_CARGO_TARGET_SWEEP:-1}" == "0" && "${MEI_CARGO_TARGET_GC:-1}" == "0" ]]; then
    return 0
  fi
  if [[ ! -d "${target_dir}" ]]; then
    return 0
  fi

  max_bytes="$(_cargo_target_gc_max_bytes)"
  human_max="$(_cargo_target_gc_human_bytes "${max_bytes}")"
  if ! before_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
    echo "warn: cargo target hygiene skipped; could not measure ${target_dir}" >&2
    return 0
  fi
  human_before="$(_cargo_target_gc_human_bytes "${before_bytes}")"

  if [[ "${MEI_CARGO_TARGET_SWEEP:-1}" == "1" ]]; then
    sweep_on="on"
  else
    sweep_on="off"
  fi
  if [[ "${MEI_CARGO_TARGET_GC:-1}" == "1" ]]; then
    clean_on="on"
  else
    clean_on="off"
  fi

  echo "==> cargo target hygiene" >&2
  echo "    dir: ${target_dir}" >&2
  echo "    current: ${human_before} | budget: ${human_max} | sweep: ${sweep_on} | clean: ${clean_on}" >&2

  after_sweep_bytes="$(maybe_cargo_target_sweep_stale "${mei_lang_root}" "${target_dir}" "${before_bytes}")"
  if (( after_sweep_bytes > max_bytes )); then
    after_sweep_bytes="$(maybe_cargo_target_sweep_incremental "${mei_lang_root}" "${target_dir}" "${after_sweep_bytes}")"
  fi
  maybe_cargo_target_gc "${mei_lang_root}" "${target_dir}" "${max_bytes}" "${after_sweep_bytes}"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  mei_lang_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  maybe_cargo_target_hygiene "${mei_lang_root}"
fi
