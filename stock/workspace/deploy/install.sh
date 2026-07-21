#!/usr/bin/env bash
# Install mei-compiler + mei-plug-ds + mei-host-shell + mei-app-runtime into deploy/bin/.
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP=""
PROFILE="debug"
FROM="lang"
RELEASE_TAG="${MEI_RELEASE_TAG:-darwin-arm64-local}"
RELEASE_VERSION="${MEI_RELEASE_VERSION:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
    --profile) PROFILE="$2"; shift 2 ;;
    --profile=*) PROFILE="${1#*=}"; shift ;;
    --from) FROM="$2"; shift 2 ;;
    --from=*) FROM="${1#*=}"; shift ;;
    --tag) RELEASE_TAG="$2"; shift 2 ;;
    --tag=*) RELEASE_TAG="${1#*=}"; shift ;;
    --version) RELEASE_VERSION="$2"; shift 2 ;;
    --version=*) RELEASE_VERSION="${1#*=}"; shift ;;
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

case "${PROFILE}" in
  debug|release) ;;
  *)
    echo "error: profile must be debug or release (got ${PROFILE})" >&2
    exit 1
    ;;
esac

case "${FROM}" in
  lang|release) ;;
  *)
    echo "error: --from must be lang or release (got ${FROM})" >&2
    exit 1
    ;;
esac

BIN_DIR="${WORKSPACE_ROOT}/deploy/bin"
mkdir -p "${BIN_DIR}"

old_version=""
if [[ -x "${BIN_DIR}/mei-host-shell" ]]; then
  old_version="$("${BIN_DIR}/mei-host-shell" -V 2>/dev/null | head -1 || true)"
fi

install_from_lang() {
  local mei_lang_root target_dir subdir build_script build_env
  mei_lang_root="$(resolve_mei_lang_root "${WORKSPACE_ROOT}")"
  target_dir="$(cargo_target_dir "${WORKSPACE_ROOT}")"
  subdir="$(profile_target_subdir)"
  build_script="${mei_lang_root}/scripts/build/build.sh"
  build_env="${mei_lang_root}/scripts/build/build-env.sh"

  echo "==> building from mei-lang (profile=${PROFILE}, root=${mei_lang_root})"
  if [[ -f "${build_script}" ]]; then
    if [[ "${PROFILE}" == "release" ]]; then
      MEI_CARGO_TARGET_HYGIENE=1 CARGO_TARGET_DIR="${target_dir}" "${build_script}" --release
    else
      MEI_CARGO_TARGET_HYGIENE=1 CARGO_TARGET_DIR="${target_dir}" "${build_script}" --debug
    fi
  else
    # shellcheck source=/dev/null
    source "${mei_lang_root}/scripts/ops/cargo-target-gc.sh"
    # shellcheck source=/dev/null
    source "${build_env}"
    mei_export_duckdb_prebuilt "${mei_lang_root}"
    maybe_cargo_target_hygiene "${mei_lang_root}"
    local cargo_args=(build --manifest-path "${mei_lang_root}/Cargo.toml" \
      -p mei-compiler -p mei-plug-ds -p mei-host-shell -p mei-app-runtime)
    if [[ "${PROFILE}" == "release" ]]; then
      cargo_args=(build --release --manifest-path "${mei_lang_root}/Cargo.toml" \
        -p mei-compiler -p mei-plug-ds -p mei-host-shell -p mei-app-runtime)
    fi
    CARGO_TARGET_DIR="${target_dir}" cargo "${cargo_args[@]}"
  fi

  for name in mei-compiler mei-plug-ds mei-host-shell mei-app-runtime; do
    local src="${target_dir}/${subdir}/${name}"
    if [[ ! -x "${src}" ]]; then
      echo "error: binary missing after build: ${src}" >&2
      exit 1
    fi
    ln -sfn "${src}" "${BIN_DIR}/${name}"
  done

  # Query engine is DataFusion (in-process). Drop any stale libduckdb beside bins.
  # shellcheck source=/dev/null
  source "${build_env}"
  mei_install_libduckdb_beside "${target_dir}/${subdir}"
  mei_install_libduckdb_beside "${BIN_DIR}"

  write_runtime_json "${WORKSPACE_ROOT}" "lang"
}

resolve_release_bundle_dir() {
  local mei_release_root bundle_root
  mei_release_root="$(resolve_mei_release_root "${WORKSPACE_ROOT}")"
  if [[ -z "${RELEASE_VERSION}" ]]; then
    echo "error: --from release requires --version <mei-lang-version>" >&2
    echo "hint: set MEI_RELEASE_VERSION or pass --version" >&2
    exit 1
  fi
  bundle_root="${mei_release_root}/targets/${RELEASE_TAG}/${RELEASE_VERSION}"
  if [[ ! -d "${bundle_root}" ]]; then
    echo "error: release bundle not found: ${bundle_root}" >&2
    exit 1
  fi
  printf '%s' "${bundle_root}"
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

install_from_release() {
  local bundle_root name src
  bundle_root="$(resolve_release_bundle_dir)"
  echo "==> installing from mei-release (tag=${RELEASE_TAG}, version=${RELEASE_VERSION})"
  echo "    bundle=${bundle_root}"

  for name in mei-compiler mei-plug-ds mei-host-shell mei-app-runtime; do
    if ! src="$(find_release_binary "${bundle_root}" "${name}")"; then
      echo "error: ${name} not found under ${bundle_root}" >&2
      echo "hint: mei-release v2 bundle layout: targets/<tag>/<version>/v2/bin/{mei-host-shell,...}" >&2
      exit 1
    fi
    ln -sfn "${src}" "${BIN_DIR}/${name}"
  done

  # Stale libduckdb from older bundles must not ship beside DataFusion bins.
  rm -f "${BIN_DIR}/libduckdb.so" "${BIN_DIR}/libduckdb.dylib" "${BIN_DIR}/duckdb.dll" 2>/dev/null || true

  mkdir -p "${WORKSPACE_ROOT}/deploy"
  cat >"$(runtime_json_path "${WORKSPACE_ROOT}")" <<EOF
{
  "profile": "${PROFILE}",
  "from": "release",
  "releaseTag": "${RELEASE_TAG}",
  "releaseVersion": "${RELEASE_VERSION}",
  "bundleDir": "${bundle_root}",
  "installedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
}

if [[ "${FROM}" == "lang" ]]; then
  install_from_lang
else
  install_from_release
fi

new_version="$("${BIN_DIR}/mei-host-shell" -V 2>/dev/null | head -1 || true)"
echo "Installed to ${BIN_DIR} (profile=${PROFILE}, from=${FROM})"
if [[ -n "${old_version}" ]]; then
  echo "Previous: ${old_version}"
fi
if [[ -n "${new_version}" ]]; then
  echo "Current:  ${new_version}"
fi

SOURCE="installed"
PROFILE="${PROFILE}"
apply_runtime_env_from_flags

if [[ -n "${old_version}" && -n "${new_version}" && "${old_version}" != "${new_version}" ]]; then
  echo "==> mei-lang version changed; aligning workspace env generation"
  apply_workspace_deploy_env "${WORKSPACE_ROOT}"
  APP="$(resolve_default_app "${WORKSPACE_ROOT}")"
  ensure_build_generation_aligned "${WORKSPACE_ROOT}" "${APP}"
fi

echo ""
echo "CLI (from workspace root):"
echo "  ./deploy/mei-host-shell -V"
echo "  ./deploy/dev.sh              # debug serve"
echo "  ./deploy/dev.sh --cargo      # debug from target/debug"
echo "  ./deploy/prod.sh             # release serve (install --release first)"
echo ""
