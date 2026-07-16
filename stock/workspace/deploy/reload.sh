#!/usr/bin/env bash
# Compile .mei sources and import into running host registry.
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
# shellcheck source=lib.sh
source "${DEPLOY_DIR}/lib.sh"

APP="${MEI_APP:-zhifa}"
parse_common_args "$@"

# Reload operates in the running app's active env/current generation.
# Do not run `build prepare` here: same-generation prepare has replace
# semantics and would delete var/data-snapshots before Access rewarms.
if [[ ! -L "${WORKSPACE_ROOT}/apps/${APP}/env/current" ]]; then
  echo "error: reload requires apps/${APP}/env/current; start or prebuild the app first" >&2
  exit 1
fi

echo "==> compile"
run_mei_compiler "${WORKSPACE_ROOT}" \
  compile --workspace "${WORKSPACE_ROOT}" --app "${APP}"

echo "==> reload (import)"
run_mei_host_shell "${WORKSPACE_ROOT}" \
  reload --workspace "${WORKSPACE_ROOT}" --app "${APP}" \
  ${DEPLOY_CLI_ARGS[@]+"${DEPLOY_CLI_ARGS[@]}"}
