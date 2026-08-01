#!/usr/bin/env bash
# 启动 Host（二进制来自 mei-env）。默认 profile=debug；--release 或 --profile release。
#
# 用法：
#   ./deploy/start-host.sh --app mini-data
#   ./deploy/start-host.sh --launch --host 127.0.0.1 --port 9527
#   # 或在 workspace.json 写 "workspace": { "port": 19527, "listenHost": "127.0.0.1" }
#   # 优先级：--port/--host > MEI_PORT/MEI_SERVE_HOST > workspace.json > 默认 9527 / 127.0.0.1
#   ./deploy/start-host.sh --app zhifa --skip-prebuild
set -euo pipefail

# Workspace identity must come from thin deploy/*.sh entry.
if [[ -z "${MEI_WORKSPACE_ROOT:-}" || -z "${MEI_WORKSPACE_DEPLOY_DIR:-}" ]]; then
  echo "error: run via workspace ./deploy/<entry>.sh (thin shell), not stock/impl directly" >&2
  exit 1
fi
DEPLOY_DIR="${MEI_WORKSPACE_DEPLOY_DIR}"
WORKSPACE_ROOT="${MEI_WORKSPACE_ROOT}"
STOCK_DEPLOY="${MEI_STOCK_DEPLOY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
# shellcheck source=../lib.sh
source "${STOCK_DEPLOY}/lib.sh"
export MEI_DEPLOY_LIB_PATH="${STOCK_DEPLOY}/lib.sh"

PROFILE="${MEI_PROFILE:-debug}"
export MEI_PROFILE="${PROFILE}"
SOURCE="mei-env"
RUNTIME="mei-env"
export MEI_SOURCE="${SOURCE}" MEI_RUNTIME="${RUNTIME}"

run_workspace_serve "${WORKSPACE_ROOT}" "$@"
