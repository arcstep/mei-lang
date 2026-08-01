#!/usr/bin/env bash
# ══════════════════════════════════════════════════════════════════
# 工作区薄入口 — publish
#
# 身份真源（本仓可改）: deploy/ops/target.env
# 共享启动器:           mei-env/release/sync/ops-publish.sh
# 发布引擎:             mei-env/release/sync/publish-v2-host.sh
# 策略:                 deploy/.publish
#
# 勿在此复制 MEI_PUBLISH_*；改目标只编辑 target.env
# ══════════════════════════════════════════════════════════════════
set -euo pipefail
OPS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${OPS_DIR}/../.." && pwd)"
MONOREPO_ROOT="$(cd "${WORKSPACE_ROOT}/../.." && pwd)"
MEI_ENV_ROOT="${MEI_ENV_ROOT:-${MONOREPO_ROOT}/mei-env}"
export MEI_WORKSPACE_SOURCE_ROOT="${WORKSPACE_ROOT}"
exec "${MEI_ENV_ROOT}/release/sync/ops-publish.sh" "$@"
