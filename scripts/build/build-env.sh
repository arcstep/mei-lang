#!/usr/bin/env bash
# Shared environment for managed MeiLang Cargo builds.

mei_cargo_target_dir() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local canonical="${MEI_CARGO_TARGET_DIR:-${mei_lang_root}/target}"
  if [[ -n "${CARGO_TARGET_DIR:-}" && "${CARGO_TARGET_DIR}" != "${canonical}" ]]; then
    echo "warn: ignoring inherited CARGO_TARGET_DIR=${CARGO_TARGET_DIR}" >&2
    echo "      managed builds use ${canonical}; set MEI_CARGO_TARGET_DIR to override" >&2
  fi
  printf '%s' "${canonical}"
}

mei_export_build_identity() {
  local mei_lang_root="${1:?mei_lang_root required}"
  local value

  if [[ -z "${MEI_GIT_COMMIT_SHORT:-}" ]]; then
    value="$(git -C "${mei_lang_root}" rev-parse --short HEAD 2>/dev/null || true)"
    export MEI_GIT_COMMIT_SHORT="${value:-unknown}"
  fi
  if [[ -z "${MEI_GIT_COMMIT_FULL:-}" ]]; then
    value="$(git -C "${mei_lang_root}" rev-parse HEAD 2>/dev/null || true)"
    export MEI_GIT_COMMIT_FULL="${value:-unknown}"
  fi
  if [[ -z "${MEI_GIT_BRANCH:-}" ]]; then
    value="$(git -C "${mei_lang_root}" rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
    export MEI_GIT_BRANCH="${value:-unknown}"
  fi
  if [[ -z "${MEI_GIT_DIRTY:-}" ]]; then
    value="$(git -C "${mei_lang_root}" status --porcelain 2>/dev/null || true)"
    if [[ -n "${value}" ]]; then
      export MEI_GIT_DIRTY=true
    else
      export MEI_GIT_DIRTY=false
    fi
  fi
  if [[ -z "${MEI_BUILD_TIMESTAMP_UTC:-}" ]]; then
    value="$(git -C "${mei_lang_root}" log -1 --format=%cI 2>/dev/null || true)"
    export MEI_BUILD_TIMESTAMP_UTC="${value:-unknown}"
  fi
}

# Historical no-ops: query engine is DataFusion (pure Rust). libduckdb is not delivered.
mei_export_duckdb_prebuilt() {
  echo "==> query engine: DataFusion (no libduckdb)" >&2
  return 0
}

mei_install_libduckdb_beside() {
  local dest_dir="${1:?dest dir required}"
  mkdir -p "${dest_dir}"
  # Remove stale libduckdb copies from older installs beside bins.
  rm -f "${dest_dir}/libduckdb.dylib" "${dest_dir}/libduckdb.so" "${dest_dir}/duckdb.dll" 2>/dev/null || true
  return 0
}
