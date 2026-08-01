#!/usr/bin/env bash
# Deprecated thin forwarder → mei.sh init (SSOT 0608). Prefer:
#   ./scripts/env/mei.sh init …
set -euo pipefail
ENV_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "warn: bootstrap.sh is deprecated; use mei.sh init" >&2
exec bash "${ENV_DIR}/mei.sh" init "$@"
