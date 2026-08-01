#!/usr/bin/env bash
# Build/fill toolchain into mei-env then install into deploy/bin/ (does not start serve).
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

PROFILE="debug"
FROM="lang"
RELEASE_TAG="${MEI_RELEASE_TAG:-darwin-arm64-local}"
RELEASE_VERSION="${MEI_RELEASE_VERSION:-}"
DO_COPY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    --debug) PROFILE="debug"; shift ;;
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

case "${FROM}" in
  lang|release|env|mei-env) ;;
  *)
    echo "error: --from must be lang|env|release (got ${FROM})" >&2
    exit 1
    ;;
esac

if [[ "${FROM}" == "lang" ]]; then
  "${DEPLOY_DIR}/fill.sh" --profile "${PROFILE}" --tag "${RELEASE_TAG}"
  FROM="env"
fi

INSTALL_ARGS=(--from "${FROM}" --profile "${PROFILE}" --tag "${RELEASE_TAG}")
[[ -n "${RELEASE_VERSION}" ]] && INSTALL_ARGS+=(--version "${RELEASE_VERSION}")
[[ "${DO_COPY}" -eq 1 ]] && INSTALL_ARGS+=(--copy)

"${DEPLOY_DIR}/install.sh" "${INSTALL_ARGS[@]}"
