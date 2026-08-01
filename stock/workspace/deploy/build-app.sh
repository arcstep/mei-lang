#!/usr/bin/env bash
# ══════════════════════════════════════════════════════════════════
# 工作区薄入口 — build-app
# 实现: $MEI_STOCK_DEPLOY/impl/build-app.sh
# ══════════════════════════════════════════════════════════════════
set -euo pipefail
DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${DEPLOY_DIR}/.." && pwd)"
export MEI_WORKSPACE_ROOT="${WORKSPACE_ROOT}"
export MEI_WORKSPACE_DEPLOY_DIR="${DEPLOY_DIR}"

if [[ -z "${MEI_STOCK_DEPLOY:-}" ]]; then
  if [[ -n "${MEI_LANG_ROOT:-}" && -d "${MEI_LANG_ROOT}/stock/workspace/deploy/impl" ]]; then
    MEI_STOCK_DEPLOY="${MEI_LANG_ROOT}/stock/workspace/deploy"
  elif [[ -d "${WORKSPACE_ROOT}/../../mei-lang/stock/workspace/deploy/impl" ]]; then
    MEI_STOCK_DEPLOY="$(cd "${WORKSPACE_ROOT}/../../mei-lang/stock/workspace/deploy" && pwd)"
  else
    echo "error: set MEI_STOCK_DEPLOY or MEI_LANG_ROOT to mei-lang stock/workspace/deploy" >&2
    exit 1
  fi
fi
export MEI_STOCK_DEPLOY
exec bash "${MEI_STOCK_DEPLOY}/impl/build-app.sh" "$@"
