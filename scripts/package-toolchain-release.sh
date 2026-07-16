#!/usr/bin/env bash
# Compatibility wrapper; new release automation packages both runtime and toolchain.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${SCRIPT_DIR}/package-release-bundles.sh" --product toolchain "$@"
