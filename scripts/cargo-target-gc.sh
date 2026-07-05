#!/usr/bin/env bash
# Cargo target hygiene: sweep stale artifacts, then `cargo clean` when over budget.
#
# Env:
#   MEI_CARGO_TARGET_GC=0           disable auto clean (default: 1)
#   MEI_CARGO_TARGET_SWEEP=0          disable stale-artifact sweep (default: 1)
#   MEI_CARGO_TARGET_MAX_GB=10         budget in GiB (default: 10)
#   MEI_CARGO_TARGET_MAX_BYTES=…      override budget in bytes
#   MEI_CARGO_TARGET_GC_DRY_RUN=1     print action only, do not clean
#   MEI_CARGO_SWEEP_KEEP_PKGS=…       runtime closure roots (default: mei-compiler,mei-plug-ds,mei-host-shell)
#   MEI_CARGO_BUILD_PROFILE=debug|release  active build profile (default: debug)
#   MEI_CARGO_TARGET_HYGIENE=1         run hygiene before build (default: 1; set 0 to disable)
#   MEI_CARGO_TARGET_DEFER_CLEAN=1     over budget after safe steps: warn only, skip cargo clean
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
  local max_gb="${MEI_CARGO_TARGET_MAX_GB:-10}"
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

  if [[ "${phase}" == "profile-drop" ]]; then
    python3 "${sweep_py}" "${target_dir}" "${dry_run_args[@]}" \
      --active-profile "${active_profile}" \
      --profile-drop-only \
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

_cargo_target_reclaimed_bytes() {
  local before_bytes="$1"
  local after_bytes="$2"
  local reclaimed=$((before_bytes - after_bytes))
  if (( reclaimed < 0 )); then
    reclaimed=0
  fi
  printf '%s' "${reclaimed}"
}

_cargo_target_emit_summary_banner() {
  local title="$1"
  shift
  local width=58
  local border=""
  local i
  for ((i = 0; i < width; i++)); do
    border+="═"
  done
  echo "" >&2
  echo "${border}" >&2
  echo "  ✓ ${title}" >&2
  while [[ $# -gt 0 ]]; do
    echo "  $1" >&2
    shift
  done
  echo "${border}" >&2
  echo "" >&2
}

_cargo_target_optional_dir_size_bytes() {
  local path="$1"
  if [[ -d "${path}" ]]; then
    _cargo_target_dir_size_bytes "${path}"
  else
    printf '0'
  fi
}

_cargo_target_budget_status_label() {
  local size_bytes="$1"
  local max_bytes="$2"
  if (( size_bytes <= max_bytes )); then
    printf '%s' "within budget"
  else
    printf '%s' "over budget"
  fi
}

# Populate inspect lines on stdout (one per line) for banner embedding.
_cargo_target_inspect_lines() {
  local target_dir="$1"
  local active_profile="${2:-debug}"
  local max_bytes total_bytes debug_bytes release_bytes active_bytes
  local human_total human_max human_debug human_release human_active budget_label

  max_bytes="$(_cargo_target_gc_max_bytes)"
  total_bytes="$(_cargo_target_optional_dir_size_bytes "${target_dir}")"
  debug_bytes="$(_cargo_target_optional_dir_size_bytes "${target_dir}/debug")"
  release_bytes="$(_cargo_target_optional_dir_size_bytes "${target_dir}/release")"
  if [[ "${active_profile}" == "release" ]]; then
    active_bytes="${release_bytes}"
  else
    active_bytes="${debug_bytes}"
  fi

  human_total="$(_cargo_target_gc_human_bytes "${total_bytes}")"
  human_max="$(_cargo_target_gc_human_bytes "${max_bytes}")"
  human_debug="$(_cargo_target_gc_human_bytes "${debug_bytes}")"
  human_release="$(_cargo_target_gc_human_bytes "${release_bytes}")"
  human_active="$(_cargo_target_gc_human_bytes "${active_bytes}")"
  budget_label="$(_cargo_target_budget_status_label "${total_bytes}" "${max_bytes}")"

  printf 'dir: %s\n' "${target_dir}"
  printf 'total: %s / budget %s (%s)\n' "${human_total}" "${human_max}" "${budget_label}"
  if [[ -d "${target_dir}/release" ]]; then
    printf 'profile debug: %s | release: %s | active(%s): %s\n' \
      "${human_debug}" "${human_release}" "${active_profile}" "${human_active}"
  else
    printf 'profile debug: %s | release: (absent) | active(%s): %s\n' \
      "${human_debug}" "${active_profile}" "${human_active}"
  fi
}

_cargo_target_record_hygiene_summary() {
  local outcome="$1"
  local before_bytes="$2"
  local after_bytes="$3"
  local detail="${4:-}"
  local human_before human_after human_max human_reclaimed reclaimed max_bytes

  max_bytes="$(_cargo_target_gc_max_bytes)"
  human_before="$(_cargo_target_gc_human_bytes "${before_bytes}")"
  human_after="$(_cargo_target_gc_human_bytes "${after_bytes}")"
  human_max="$(_cargo_target_gc_human_bytes "${max_bytes}")"
  reclaimed="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  human_reclaimed="$(_cargo_target_gc_human_bytes "${reclaimed}")"

  case "${outcome}" in
    under-budget)
      export MEI_CARGO_TARGET_HYGIENE_SUMMARY="not needed (${human_before} <= budget ${human_max})"
      ;;
    disabled)
      export MEI_CARGO_TARGET_HYGIENE_SUMMARY="disabled (${detail})"
      ;;
    deferred)
      export MEI_CARGO_TARGET_HYGIENE_SUMMARY="reclaimed ${human_reclaimed} (${human_before} -> ${human_after}); clean deferred (still over ${human_max})"
      ;;
    completed)
      if (( reclaimed > 0 )); then
        export MEI_CARGO_TARGET_HYGIENE_SUMMARY="reclaimed ${human_reclaimed} (${human_before} -> ${human_after})"
      else
        export MEI_CARGO_TARGET_HYGIENE_SUMMARY="completed (${human_after} vs budget ${human_max})"
      fi
      ;;
    *)
      export MEI_CARGO_TARGET_HYGIENE_SUMMARY="${outcome}"
      ;;
  esac
}

_cargo_target_finish_hygiene_report() {
  local target_dir="$1"
  local active_profile="$2"
  local before_bytes="$3"
  local after_bytes="$4"
  local outcome="$5"
  local detail="${6:-}"

  _cargo_target_record_hygiene_summary "${outcome}" "${before_bytes}" "${after_bytes}" "${detail}"
  if [[ "${MEI_CARGO_TARGET_EMIT_HYGIENE_BANNER:-0}" == "1" ]]; then
    _cargo_target_emit_hygiene_result_banner "${target_dir}" "${active_profile}" \
      "${before_bytes}" "${after_bytes}" "${outcome}" "${detail}"
  fi
}

cargo_target_emit_startup_panel() {
  local target_dir="${1:?target_dir required}"
  local active_profile="${2:-debug}"
  local build_plan="${3:-compile}"
  local hygiene_note="${4:-}"
  local title hygiene_line build_line
  local workspace_root="${5:-}"
  local bin_name bin_path inspect_line
  local lines=() banner_lines=()

  while IFS= read -r inspect_line; do
    lines+=("${inspect_line}")
  done < <(_cargo_target_inspect_lines "${target_dir}" "${active_profile}")

  if [[ -n "${MEI_CARGO_TARGET_HYGIENE_SUMMARY:-}" ]]; then
    hygiene_line="${MEI_CARGO_TARGET_HYGIENE_SUMMARY}"
  elif [[ -n "${hygiene_note}" ]]; then
    hygiene_line="${hygiene_note}"
  elif [[ "${MEI_CARGO_TARGET_HYGIENE:-1}" == "0" ]]; then
    hygiene_line="disabled (MEI_CARGO_TARGET_HYGIENE=0)"
  elif [[ "${MEI_CARGO_TARGET_SWEEP:-1}" == "0" && "${MEI_CARGO_TARGET_GC:-1}" == "0" ]]; then
    hygiene_line="disabled (MEI_CARGO_TARGET_SWEEP=0 and MEI_CARGO_TARGET_GC=0)"
  else
    local total_bytes max_bytes
    total_bytes="$(_cargo_target_optional_dir_size_bytes "${target_dir}")"
    max_bytes="$(_cargo_target_gc_max_bytes)"
    if (( total_bytes > max_bytes )); then
      hygiene_line="will run (over budget — safe sweep then cargo clean if needed)"
    else
      hygiene_line="not needed (within budget)"
    fi
  fi

  case "${build_plan}" in
    skip)
      build_line="skipped (MEI_CARGO_SKIP_BUILD_IF_FRESH=1; binaries present)"
      local total_bytes max_bytes
      total_bytes="$(_cargo_target_optional_dir_size_bytes "${target_dir}")"
      max_bytes="$(_cargo_target_gc_max_bytes)"
      if (( total_bytes > max_bytes )); then
        title="编译缓存检查 · CARGO TARGET OVER BUDGET"
      else
        title="编译缓存检查 · CARGO TARGET OK"
      fi
      ;;
    force-clean)
      build_line="cargo clean + build (--force-build; full rebuild)"
      title="编译与缓存 · CARGO RUNTIME FORCE BUILD"
      ;;
    compile)
      build_line="cargo build (incremental — Cargo rebuilds only changed crates)"
      title="编译与缓存 · CARGO RUNTIME BUILD"
      ;;
    *)
      build_line="${build_plan}"
      title="编译与缓存 · CARGO RUNTIME"
      ;;
  esac

  local banner_lines=("${lines[@]}" "hygiene: ${hygiene_line}" "build: ${build_line}")

  if [[ "${build_plan}" == "skip" && -n "${workspace_root}" ]] && declare -F resolve_bin_path >/dev/null 2>&1; then
    for bin_name in mei-host-shell mei-compiler mei-plug-ds; do
      bin_path="$(resolve_bin_path "${workspace_root}" "${bin_name}")"
      if [[ -x "${bin_path}" ]]; then
        banner_lines+=("${bin_name}: ${bin_path}")
      else
        banner_lines+=("${bin_name}: missing (${bin_path})")
      fi
    done
  fi

  _cargo_target_emit_summary_banner "${title}" "${banner_lines[@]}"
}

_cargo_target_emit_hygiene_result_banner() {
  local target_dir="$1"
  local active_profile="$2"
  local before_bytes="$3"
  local after_bytes="$4"
  local outcome="$5"
  local detail="${6:-}"
  local max_bytes human_before human_after human_max human_reclaimed reclaimed
  local inspect_lines=() inspect_line

  max_bytes="$(_cargo_target_gc_max_bytes)"
  human_before="$(_cargo_target_gc_human_bytes "${before_bytes}")"
  human_after="$(_cargo_target_gc_human_bytes "${after_bytes}")"
  human_max="$(_cargo_target_gc_human_bytes "${max_bytes}")"
  reclaimed="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  human_reclaimed="$(_cargo_target_gc_human_bytes "${reclaimed}")"

  while IFS= read -r inspect_line; do
    inspect_lines+=("${inspect_line}")
  done < <(_cargo_target_inspect_lines "${target_dir}" "${active_profile}")

  case "${outcome}" in
    under-budget)
      _cargo_target_emit_summary_banner "编译缓存治理 · CARGO TARGET OK" \
        "${inspect_lines[@]}" \
        "hygiene: not needed (${human_before} <= budget ${human_max})" \
        "action: left target untouched"
      ;;
    completed)
      _cargo_target_emit_summary_banner "编译缓存治理 · CARGO TARGET HYGIENE" \
        "${inspect_lines[@]}" \
        "hygiene: reclaimed ${human_reclaimed} (${human_before} -> ${human_after})" \
        "budget: ${human_max} (${human_after} vs ${human_max})" \
        "${detail}"
      ;;
    deferred)
      _cargo_target_emit_summary_banner "编译缓存治理 · CARGO TARGET OVER BUDGET" \
        "${inspect_lines[@]}" \
        "hygiene: reclaimed ${human_reclaimed} (${human_before} -> ${human_after})" \
        "budget: ${human_max} (still over — live debug closure cannot shrink further without clean)" \
        "action: cargo clean skipped (binaries present); run mei-lang/scripts/cargo-target-gc.sh with MEI_CARGO_TARGET_DEFER_CLEAN=0" \
        "${detail}"
      ;;
    disabled)
      _cargo_target_emit_summary_banner "编译缓存治理 · CARGO TARGET" \
        "${inspect_lines[@]}" \
        "hygiene: disabled" \
        "${detail}"
      ;;
  esac
}

maybe_cargo_target_sweep_stale() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${2:?target_dir required}"
  local before_bytes="$3"
  local freed_bytes reclaimed_bytes human estimate_human before_human after_bytes after_human measured_after

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
    echo "    sweep: removing safe artifacts (stale hashes, link .o, inactive profile, tests; current ${before_human})" >&2
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

  measured_after=0
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    after_bytes=$((before_bytes - freed_bytes))
    if (( after_bytes < 0 )); then
      after_bytes=0
    fi
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  elif after_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
    measured_after=1
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  else
    after_bytes="$((before_bytes - freed_bytes))"
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  fi
  after_human="$(_cargo_target_gc_human_bytes "${after_bytes}")"
  human="$(_cargo_target_gc_human_bytes "${reclaimed_bytes}")"

  if (( freed_bytes <= 0 )); then
    echo "    sweep: no stale artifacts (${after_human}, unchanged)" >&2
    printf '%s' "${after_bytes}"
    return 0
  fi

  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "    sweep: would reclaim ${human} (${before_human} -> ${after_human})" >&2
  else
    echo "    sweep: reclaimed ${human} (${before_human} -> ${after_human})" >&2
    if (( measured_after == 1 && freed_bytes > reclaimed_bytes )); then
      estimate_human="$(_cargo_target_gc_human_bytes "${freed_bytes}")"
      echo "    sweep: deleted paths summed to ${estimate_human}; lower du delta is normal (hardlinks / shared blocks)" >&2
    fi
  fi
  printf '%s' "${after_bytes}"
}

maybe_cargo_target_sweep_inactive_profile() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${2:?target_dir required}"
  local before_bytes="$3"
  local freed_bytes reclaimed_bytes human estimate_human before_human after_bytes after_human measured_after

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
    echo "    sweep(profile): dry-run inactive profile drop (current ${before_human})" >&2
  else
    echo "    sweep(profile): still over budget; dropping inactive profile (current ${before_human})" >&2
  fi

  freed_bytes="$(_cargo_target_sweep_stale_bytes "${target_dir}" "${mei_lang_root}" profile-drop)"
  if [[ "${freed_bytes}" == "missing" ]]; then
    printf '%s' "${before_bytes}"
    return 0
  fi
  if [[ -z "${freed_bytes}" || ! "${freed_bytes}" =~ ^[0-9]+$ ]]; then
    echo "    sweep(profile): skipped (could not measure reclaimed bytes)" >&2
    printf '%s' "${before_bytes}"
    return 0
  fi

  measured_after=0
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    after_bytes=$((before_bytes - freed_bytes))
    if (( after_bytes < 0 )); then
      after_bytes=0
    fi
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  elif after_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
    measured_after=1
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  else
    after_bytes="$((before_bytes - freed_bytes))"
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  fi
  after_human="$(_cargo_target_gc_human_bytes "${after_bytes}")"
  human="$(_cargo_target_gc_human_bytes "${reclaimed_bytes}")"

  if (( freed_bytes <= 0 )); then
    echo "    sweep(profile): no inactive profile directory (${after_human}, unchanged)" >&2
    printf '%s' "${after_bytes}"
    return 0
  fi

  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "    sweep(profile): would reclaim ${human} (${before_human} -> ${after_human})" >&2
  else
    echo "    sweep(profile): reclaimed ${human} (${before_human} -> ${after_human})" >&2
    if (( measured_after == 1 && freed_bytes > reclaimed_bytes )); then
      estimate_human="$(_cargo_target_gc_human_bytes "${freed_bytes}")"
      echo "    sweep(profile): deleted paths summed to ${estimate_human}; lower du delta is normal (hardlinks / shared blocks)" >&2
    fi
  fi
  printf '%s' "${after_bytes}"
}

maybe_cargo_target_sweep_incremental() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${2:?target_dir required}"
  local before_bytes="$3"
  local freed_bytes reclaimed_bytes human estimate_human before_human after_bytes after_human measured_after

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

  measured_after=0
  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    after_bytes=$((before_bytes - freed_bytes))
    if (( after_bytes < 0 )); then
      after_bytes=0
    fi
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  elif after_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
    measured_after=1
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  else
    after_bytes="$((before_bytes - freed_bytes))"
    reclaimed_bytes="$(_cargo_target_reclaimed_bytes "${before_bytes}" "${after_bytes}")"
  fi
  after_human="$(_cargo_target_gc_human_bytes "${after_bytes}")"
  human="$(_cargo_target_gc_human_bytes "${reclaimed_bytes}")"

  if (( freed_bytes <= 0 )); then
    echo "    sweep(incremental): no incremental cache (${after_human}, unchanged)" >&2
    printf '%s' "${after_bytes}"
    return 0
  fi

  if [[ "${MEI_CARGO_TARGET_GC_DRY_RUN:-0}" == "1" ]]; then
    echo "    sweep(incremental): would reclaim ${human} (${before_human} -> ${after_human})" >&2
  else
    echo "    sweep(incremental): reclaimed ${human} (${before_human} -> ${after_human})" >&2
    if (( measured_after == 1 && freed_bytes > reclaimed_bytes )); then
      estimate_human="$(_cargo_target_gc_human_bytes "${freed_bytes}")"
      echo "    sweep(incremental): deleted paths summed to ${estimate_human}; lower du delta is normal (hardlinks / shared blocks)" >&2
    fi
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

# Run before cargo build when target exceeds budget:
#   1) safe sweep — stale hashes, link intermediates (.o), out-of-scope, tests, inactive profile
#   2) incremental wipe — slower next edit-compile, not a cold rebuild
#   3) cargo clean — only if still over budget and MEI_CARGO_TARGET_DEFER_CLEAN!=1
maybe_cargo_target_hygiene() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local target_dir="${CARGO_TARGET_DIR:-${mei_lang_root}/target}"
  local max_bytes before_bytes human_before human_max sweep_on clean_on after_sweep_bytes
  local active_profile hygiene_detail outcome

  active_profile="${MEI_CARGO_BUILD_PROFILE:-debug}"
  if [[ "${active_profile}" != "debug" && "${active_profile}" != "release" ]]; then
    active_profile="debug"
  fi

  if [[ "${MEI_CARGO_TARGET_SWEEP:-1}" == "0" && "${MEI_CARGO_TARGET_GC:-1}" == "0" ]]; then
    if [[ -d "${target_dir}" ]] && before_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
      _cargo_target_finish_hygiene_report "${target_dir}" "${active_profile}" \
        "${before_bytes}" "${before_bytes}" "disabled" \
        "MEI_CARGO_TARGET_SWEEP=0 and MEI_CARGO_TARGET_GC=0"
    fi
    return 0
  fi
  if [[ ! -d "${target_dir}" ]]; then
    echo "warn: cargo target hygiene skipped; missing ${target_dir}" >&2
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

  if (( before_bytes <= max_bytes )); then
    echo "    hygiene: under budget (${human_before} <= ${human_max}), leaving target untouched" >&2
    _cargo_target_finish_hygiene_report "${target_dir}" "${active_profile}" \
      "${before_bytes}" "${before_bytes}" "under-budget"
    return 0
  fi

  outcome="completed"
  hygiene_detail="phases: safe sweep (stale hashes, link intermediates, inactive profile, tests)"

  after_sweep_bytes="$(maybe_cargo_target_sweep_stale "${mei_lang_root}" "${target_dir}" "${before_bytes}")"
  if (( after_sweep_bytes > max_bytes )); then
    hygiene_detail+=", incremental"
    after_sweep_bytes="$(maybe_cargo_target_sweep_incremental "${mei_lang_root}" "${target_dir}" "${after_sweep_bytes}")"
  fi
  if (( after_sweep_bytes > max_bytes )); then
    if [[ "${MEI_CARGO_TARGET_DEFER_CLEAN:-0}" == "1" ]]; then
      outcome="deferred"
      hygiene_detail+=", clean deferred (live debug closure still over budget)"
      echo "    clean: deferred (MEI_CARGO_TARGET_DEFER_CLEAN=1; binaries usable — run cargo-target-gc.sh or install to force clean)" >&2
    elif [[ "${MEI_CARGO_TARGET_GC:-1}" == "0" ]]; then
      hygiene_detail+=", clean skipped (MEI_CARGO_TARGET_GC=0)"
      echo "    clean: disabled (MEI_CARGO_TARGET_GC=0; still over budget after safe steps)" >&2
    else
      hygiene_detail+=", cargo clean"
      maybe_cargo_target_gc "${mei_lang_root}" "${target_dir}" "${max_bytes}" "${after_sweep_bytes}"
      if after_sweep_bytes="$(_cargo_target_dir_size_bytes "${target_dir}")"; then
        :
      else
        after_sweep_bytes=0
      fi
    fi
  else
    maybe_cargo_target_gc "${mei_lang_root}" "${target_dir}" "${max_bytes}" "${after_sweep_bytes}"
  fi

  _cargo_target_finish_hygiene_report "${target_dir}" "${active_profile}" \
    "${before_bytes}" "${after_sweep_bytes}" "${outcome}" "${hygiene_detail}"
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  mei_lang_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  target_dir="${CARGO_TARGET_DIR:-${mei_lang_root}/target}"
  active_profile="${MEI_CARGO_BUILD_PROFILE:-debug}"
  maybe_cargo_target_hygiene "${mei_lang_root}"
  cargo_target_emit_startup_panel "${target_dir}" "${active_profile}" "manual"
fi
