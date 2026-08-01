#!/usr/bin/env bash
# Fill mei-env/targets from a prebuilt bundle (scenario installed; no cargo).
# Usage:
#   MEI_ENV_ROOT=... ./fill-bundle-into-env.sh --tag <tag> --version <ver> --from <dir|tar.gz>
set -euo pipefail

ENV_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_ENV_ROOT="${MEI_ENV_ROOT:-${HOME}/.mei-env}"

TARGET_TAG=""
VERSION=""
FROM_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag) TARGET_TAG="${2:?}"; shift 2 ;;
    --tag=*) TARGET_TAG="${1#*=}"; shift ;;
    --version) VERSION="${2:?}"; shift 2 ;;
    --version=*) VERSION="${1#*=}"; shift ;;
    --from) FROM_PATH="${2:?}"; shift 2 ;;
    --from=*) FROM_PATH="${1#*=}"; shift ;;
    -h|--help)
      sed -n '2,5p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "unknown arg: $1" >&2
      exit 1
      ;;
  esac
done

if [[ -z "${TARGET_TAG}" || -z "${FROM_PATH}" ]]; then
  echo "error: require --tag and --from" >&2
  exit 1
fi

# Prefer mei-env fill-from-bundle when available
FILL_FROM_BUNDLE="${MEI_ENV_ROOT}/release/collect/fill-from-bundle.sh"
if [[ -x "${FILL_FROM_BUNDLE}" ]]; then
  if [[ -z "${VERSION}" ]]; then
    VERSION="bundle-$(date -u +%Y%m%d%H%M%S)"
  fi
  echo "==> delegating to ${FILL_FROM_BUNDLE}"
  exec env MEI_ENV_ROOT="${MEI_ENV_ROOT}" \
    "${FILL_FROM_BUNDLE}" --tag "${TARGET_TAG}" --version "${VERSION}" --from "${FROM_PATH}"
fi

"${ENV_DIR}/ensure-mei-env-root.sh" --root "${MEI_ENV_ROOT}" >/dev/null

command -v rsync >/dev/null 2>&1 || {
  echo "error: rsync not found" >&2
  exit 1
}

if [[ ! -e "${FROM_PATH}" ]]; then
  echo "error: --from path not found: ${FROM_PATH}" >&2
  exit 1
fi

WORKDIR="${FROM_PATH}"
CLEANUP=""
if [[ -f "${FROM_PATH}" ]]; then
  case "${FROM_PATH}" in
    *.tar.gz|*.tgz)
      WORKDIR="$(mktemp -d)"
      CLEANUP="${WORKDIR}"
      tar -xzf "${FROM_PATH}" -C "${WORKDIR}"
      ;;
    *)
      echo "error: unsupported archive (use directory or .tar.gz)" >&2
      exit 1
      ;;
  esac
fi

SRC_BIN=""
if [[ -d "${WORKDIR}/v2/bin" ]]; then
  SRC_BIN="${WORKDIR}/v2/bin"
elif [[ -x "${WORKDIR}/mei-host-shell" || -x "${WORKDIR}/mei-host-shell.exe" ]]; then
  SRC_BIN="${WORKDIR}"
else
  for d in "${WORKDIR}"/*; do
    [[ -d "${d}" ]] || continue
    if [[ -d "${d}/v2/bin" ]]; then
      SRC_BIN="${d}/v2/bin"
      break
    fi
    if [[ -x "${d}/mei-host-shell" || -x "${d}/mei-host-shell.exe" ]]; then
      SRC_BIN="${d}"
      break
    fi
  done
fi

if [[ -z "${SRC_BIN}" ]]; then
  echo "error: cannot find v2/bin or mei-host-shell under ${FROM_PATH}" >&2
  [[ -n "${CLEANUP}" ]] && rm -rf "${CLEANUP}"
  exit 1
fi

for name in mei-compiler mei-host-shell mei-app-runtime; do
  if [[ ! -x "${SRC_BIN}/${name}" && ! -x "${SRC_BIN}/${name}.exe" ]]; then
    echo "error: missing ${name} in ${SRC_BIN}" >&2
    [[ -n "${CLEANUP}" ]] && rm -rf "${CLEANUP}"
    exit 1
  fi
done

if [[ -z "${VERSION}" ]]; then
  VERSION="bundle-$(date -u +%Y%m%d%H%M%S)"
fi

if [[ "${VERSION}" == mei-lang-* ]]; then
  OUT_DIR="${MEI_ENV_ROOT}/targets/${TARGET_TAG}/${VERSION}"
else
  OUT_DIR="${MEI_ENV_ROOT}/targets/${TARGET_TAG}/mei-lang-${VERSION}"
fi
V2_BIN="${OUT_DIR}/v2/bin"
mkdir -p "${V2_BIN}"
echo "==> fill-bundle-into-env -> ${V2_BIN}"

for name in mei-compiler mei-host-shell mei-app-runtime; do
  if [[ -x "${SRC_BIN}/${name}" ]]; then
    rsync -a "${SRC_BIN}/${name}" "${V2_BIN}/${name}"
    chmod +x "${V2_BIN}/${name}"
  else
    rsync -a "${SRC_BIN}/${name}.exe" "${V2_BIN}/${name}.exe"
  fi
done

[[ -n "${CLEANUP}" ]] && rm -rf "${CLEANUP}"
echo "==> fill complete: ${OUT_DIR}"
echo "${OUT_DIR}"
