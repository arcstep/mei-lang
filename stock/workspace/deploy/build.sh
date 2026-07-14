#!/usr/bin/env bash
# Build mei-lang runtime binaries into deploy/bin/ (does not start serve).
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-zhifa}"
PROFILE="debug"
FROM="lang"
RELEASE_TAG="${MEI_RELEASE_TAG:-darwin-arm64-local}"
RELEASE_VERSION="${MEI_RELEASE_VERSION:-}"

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
    *) echo "unknown arg: $1" >&2; exit 1 ;;
  esac
done

case "${FROM}" in
  lang|release) ;;
  *)
    echo "error: --from must be lang or release (got ${FROM})" >&2
    exit 1
    ;;
esac

"${DEPLOY_DIR}/install.sh" --from "${FROM}" --profile "${PROFILE}" \
  ${RELEASE_VERSION:+--version "${RELEASE_VERSION}"} \
  --tag "${RELEASE_TAG}"

echo "Build complete (profile=${PROFILE}, from=${FROM})."
echo "Start: ./deploy/dev.sh   # debug"
echo "       ./deploy/prod.sh  # release (after install --release)"
