#!/usr/bin/env bash
# Install mei-compiler + mei-host-shell + mei-app-runtime into deploy/bin/ from mei-env.
#
# 默认：从已 fill 的 mei-env/targets 挂接（symlink；--copy 真复制）。
# 兼容：--from lang → 先 fill-from-lang 再安装。
#
# 见 docs/mei-lang/06-deploy-and-ops/0608-mei-env-and-scenario-bootstrap.md
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP=""
PROFILE="debug"
FROM="env"
RELEASE_TAG="${MEI_RELEASE_TAG:-darwin-arm64-local}"
RELEASE_VERSION="${MEI_RELEASE_VERSION:-}"
DO_COPY=0

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
    --copy) DO_COPY=1; shift ;;
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

# Normalize aliases
case "${FROM}" in
  env|release|mei-env) FROM="env" ;;
  lang) FROM="lang" ;;
  *)
    echo "error: --from must be env|release|lang (got ${FROM})" >&2
    exit 1
    ;;
esac

BIN_DIR="${WORKSPACE_ROOT}/deploy/bin"
mkdir -p "${BIN_DIR}"

old_version=""
if [[ -x "${BIN_DIR}/mei-host-shell" ]]; then
  old_version="$("${BIN_DIR}/mei-host-shell" -V 2>/dev/null | head -1 || true)"
fi

latest_version_under_tag() {
  local mei_env_root="$1"
  local tag="$2"
  local tag_root="${mei_env_root}/targets/${tag}"
  if [[ ! -d "${tag_root}" ]]; then
    return 1
  fi
  # Prefer directories that look like mei-lang-* version bundles
  local newest=""
  local d
  for d in "${tag_root}"/mei-lang-* "${tag_root}"/*; do
    [[ -d "${d}" ]] || continue
    [[ -x "${d}/v2/bin/mei-host-shell" || -x "${d}/bin/mei-host-shell" ]] || continue
    newest="$(basename "${d}")"
  done
  if [[ -z "${newest}" ]]; then
    return 1
  fi
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

install_link_or_copy() {
  local src="$1"
  local dest="$2"
  if [[ "${DO_COPY}" -eq 1 ]]; then
    rm -f "${dest}"
    cp -f "${src}" "${dest}"
    chmod +x "${dest}"
  else
    ln -sfn "${src}" "${dest}"
  fi
}

run_fill_from_lang() {
  local mei_env_root fill_script
  mei_env_root="$(resolve_mei_env_root "${WORKSPACE_ROOT}")"
  fill_script="${mei_env_root}/release/collect/fill-from-lang.sh"
  if [[ ! -x "${fill_script}" ]]; then
    echo "error: fill-from-lang not found: ${fill_script}" >&2
    exit 1
  fi
  local fill_args=(--tag "${RELEASE_TAG}" --profile "${PROFILE}")
  echo "==> --from lang: fill mei-env then install"
  MEI_LANG_ROOT="$(resolve_mei_lang_root "${WORKSPACE_ROOT}")" \
    MEI_ENV_ROOT="${mei_env_root}" \
    "${fill_script}" "${fill_args[@]}"
  if [[ -z "${RELEASE_VERSION}" ]]; then
    RELEASE_VERSION="$(latest_version_under_tag "${mei_env_root}" "${RELEASE_TAG}" || true)"
  fi
}

install_from_mei_env() {
  local mei_env_root bundle_root name src
  mei_env_root="$(resolve_mei_env_root "${WORKSPACE_ROOT}")"
  if [[ -z "${RELEASE_VERSION}" ]]; then
    RELEASE_VERSION="$(latest_version_under_tag "${mei_env_root}" "${RELEASE_TAG}" || true)"
  fi
  if [[ -z "${RELEASE_VERSION}" ]]; then
    echo "error: no mei-env bundle under ${mei_env_root}/targets/${RELEASE_TAG}" >&2
    echo "hint: run deploy/fill.sh or mei-env/release/collect/fill-from-lang.sh first" >&2
    echo "hint: or pass --version <dir-name under targets/tag/>" >&2
    exit 1
  fi
  bundle_root="${mei_env_root}/targets/${RELEASE_TAG}/${RELEASE_VERSION}"
  if [[ ! -d "${bundle_root}" && -d "${mei_env_root}/targets/${RELEASE_TAG}/mei-lang-${RELEASE_VERSION}" ]]; then
    bundle_root="${mei_env_root}/targets/${RELEASE_TAG}/mei-lang-${RELEASE_VERSION}"
    RELEASE_VERSION="mei-lang-${RELEASE_VERSION}"
  fi
  if [[ ! -d "${bundle_root}" ]]; then
    echo "error: release bundle not found: ${bundle_root}" >&2
    exit 1
  fi

  echo "==> installing from mei-env (tag=${RELEASE_TAG}, version=${RELEASE_VERSION}, copy=${DO_COPY})"
  echo "    bundle=${bundle_root}"

  for name in mei-compiler mei-host-shell mei-app-runtime; do
    if ! src="$(find_release_binary "${bundle_root}" "${name}")"; then
      echo "error: ${name} not found under ${bundle_root}" >&2
      echo "hint: mei-env v2 layout: targets/<tag>/<version>/v2/bin/{mei-host-shell,...}" >&2
      exit 1
    fi
    install_link_or_copy "${src}" "${BIN_DIR}/${name}"
  done

  rm -f "${BIN_DIR}/libduckdb.so" "${BIN_DIR}/libduckdb.dylib" "${BIN_DIR}/duckdb.dll" 2>/dev/null || true

  mkdir -p "${WORKSPACE_ROOT}/deploy"
  cat >"$(runtime_json_path "${WORKSPACE_ROOT}")" <<EOF
{
  "profile": "${PROFILE}",
  "from": "mei-env",
  "meiEnvRoot": "${mei_env_root}",
  "releaseTag": "${RELEASE_TAG}",
  "releaseVersion": "${RELEASE_VERSION}",
  "bundleDir": "${bundle_root}",
  "copy": $([[ "${DO_COPY}" -eq 1 ]] && echo true || echo false),
  "installedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF
}

if [[ "${FROM}" == "lang" ]]; then
  run_fill_from_lang
fi
install_from_mei_env

new_version="$("${BIN_DIR}/mei-host-shell" -V 2>/dev/null | head -1 || true)"
echo "Installed to ${BIN_DIR} (profile=${PROFILE}, from=mei-env)"
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
echo "  ./deploy/dev.sh              # debug serve (deploy/bin only)"
echo "  ./deploy/prod.sh             # release serve"
echo "  ./deploy/fill.sh             # fill mei-env (source scenarios)"
