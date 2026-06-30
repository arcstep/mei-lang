#!/usr/bin/env bash
# Start workspace in dev profile (debug binaries).
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

PROFILE="${MEI_PROFILE:-debug}"
SOURCE="${MEI_SOURCE:-installed}"
export MEI_PROFILE="${PROFILE}" MEI_SOURCE="${SOURCE}"

run_workspace_serve "${WORKSPACE_ROOT}" "$@"
