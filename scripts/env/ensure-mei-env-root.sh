#!/usr/bin/env bash
# Ensure MEI_ENV_ROOT exists with targets/ skeleton (cold-start stage 0).
# Usage: ensure-mei-env-root.sh [--root <path>]
set -euo pipefail

ROOT="${MEI_ENV_ROOT:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) ROOT="${2:?}"; shift 2 ;;
    --root=*) ROOT="${1#*=}"; shift ;;
    -h|--help)
      echo "Usage: ensure-mei-env-root.sh [--root <path>]"
      echo "Default: \$MEI_ENV_ROOT or \$HOME/.mei-env"
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "${ROOT}" ]]; then
  ROOT="${HOME}/.mei-env"
fi

mkdir -p "${ROOT}/targets"
if [[ ! -f "${ROOT}/README.md" ]]; then
  cat >"${ROOT}/README.md" <<EOF
# mei-env

Local MeiLang toolchain root (see SSOT 0608).

- \`targets/<tag>/<version>/v2/bin/\` — installed binaries
- Created by \`mei-lang/scripts/env/mei.sh init\` or monorepo \`mei-env/\`.
EOF
fi

printf '%s\n' "${ROOT}"
