#!/usr/bin/env bash
# Deprecated alias: use ./deploy/dev.sh (debug profile).
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${DEPLOY_DIR}/dev.sh" "$@"
