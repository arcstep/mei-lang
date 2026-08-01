#!/usr/bin/env bash
# mei.sh — unique user-facing entry for mei-env fill + workspace cold-start (SSOT 0608).
#
# Usage:
#   ./scripts/env/mei.sh <command> [options]
#
# Commands:
#   init              ensure mei-env → fill → optional workspace init
#   env build         compile from --source into --env (and optional --workspace pin)
#   env pin           pin meiEnv.targetTag/version on a workspace
#   workspace init    forward to stock/workspace/init-workspace.sh
#
# Location flags (replaces scenario enum):
#   --source <mei-lang root>   compile source tree (mutex with --bundle)
#   --env <mei-env root>       toolchain root (default: monorepo sibling or ~/.mei-env)
#   --bundle <dir|archive>     binary supply (alias: --from-bundle)
#
set -euo pipefail

ENV_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT_DEFAULT="$(cd "${ENV_DIR}/../.." && pwd)"

usage() {
  cat <<'EOF'
Usage: mei.sh <command> [options]

Commands:
  init              ensure mei-env → fill → optional workspace init
  env list          list target tags / versions under --env
  env build         fill mei-env from --source (cargo) and optional pin --workspace
  env pin           pin meiEnv on --workspace (requires --version)
  workspace init    initialize a workspace directory (thin deploy shells)

Common options:
  --source <path>           mei-lang source root
  --env <path>              mei-env root (also: --mei-env-root)
  --bundle <path>           prebuilt bundle (also: --from-bundle)
  --tag <target-tag>        targets/<tag>/…
  --profile debug|release   (default: debug)
  --workspace <path>        workspace directory
  --workspace-id <id>       workspace id (also: --id)
  --app <id>                first app id (init)
  --label <text>            workspace label (init)
  --no-fill                 init: skip fill
  --version <ver|latest>    pin / bundle version (latest = explicit newest)

Deprecated (mapped with warning): --scenario monorepo|lang-source|installed
EOF
}

default_target_tag() {
  case "$(uname -s)-$(uname -m)" in
    Darwin-arm64) printf '%s' "darwin-arm64-local" ;;
    Darwin-x86_64) printf '%s' "darwin-x86_64-local" ;;
    Linux-x86_64) printf '%s' "linux-x86_64-local" ;;
    Linux-aarch64) printf '%s' "linux-aarch64-local" ;;
    MINGW*|MSYS*|CYGWIN*) printf '%s' "windows-x86_64-local" ;;
    *) printf '%s' "local" ;;
  esac
}

# Detect monorepo layout from cwd or beside a mei-lang root.
detect_monorepo_env() {
  local cwd mono src="${1:-}"
  cwd="$(pwd)"
  if [[ -d "${cwd}/mei-lang" && -d "${cwd}/mei-env" ]]; then
    printf '%s' "$(cd "${cwd}/mei-env" && pwd)"
    return 0
  fi
  if [[ -n "${src}" && -d "${src}/../mei-env" && -f "${src}/Cargo.toml" ]]; then
    printf '%s' "$(cd "${src}/../mei-env" && pwd)"
    return 0
  fi
  mono="$(cd "${cwd}/.." 2>/dev/null && pwd || true)"
  if [[ -n "${mono}" && -d "${mono}/mei-lang" && -d "${mono}/mei-env" ]]; then
    printf '%s' "$(cd "${mono}/mei-env" && pwd)"
    return 0
  fi
  return 1
}

detect_default_source() {
  local cwd mono
  cwd="$(pwd)"
  if [[ -f "${cwd}/Cargo.toml" && -d "${cwd}/scripts/env" ]]; then
    printf '%s' "$(cd "${cwd}" && pwd)"
    return 0
  fi
  if [[ -d "${cwd}/mei-lang" && -f "${cwd}/mei-lang/Cargo.toml" ]]; then
    printf '%s' "$(cd "${cwd}/mei-lang" && pwd)"
    return 0
  fi
  if [[ -f "${MEI_LANG_ROOT_DEFAULT}/Cargo.toml" ]]; then
    printf '%s' "${MEI_LANG_ROOT_DEFAULT}"
    return 0
  fi
  return 1
}

latest_version_under_tag() {
  local mei_env_root="$1"
  local tag="$2"
  local tag_root="${mei_env_root}/targets/${tag}"
  [[ -d "${tag_root}" ]] || return 1
  local newest="" newest_mtime=0 d mtime
  for d in "${tag_root}"/mei-lang-* "${tag_root}"/*; do
    [[ -d "${d}" ]] || continue
    if [[ -x "${d}/v2/bin/mei-host-shell" || -x "${d}/bin/mei-host-shell" ]]; then
      mtime="$(stat -f '%m' "${d}" 2>/dev/null || stat -c '%Y' "${d}" 2>/dev/null || echo 0)"
      if [[ "${mtime}" -ge "${newest_mtime}" ]]; then
        newest_mtime="${mtime}"
        newest="$(basename "${d}")"
      fi
    fi
  done
  [[ -n "${newest}" ]] || return 1
  printf '%s' "${newest}"
}

pin_workspace_mei_env() {
  local workspace_root="$1"
  local env_root="$2"
  local tag="$3"
  local version="${4:-}"
  local ws_json="${workspace_root}/workspace.json"
  [[ -f "${ws_json}" ]] || {
    echo "error: no workspace.json at ${workspace_root}" >&2
    return 1
  }
  if [[ -z "${version}" ]]; then
    echo "error: --version is required to pin meiEnv (use env list, or --version latest)" >&2
    return 1
  fi
  if [[ "${version}" == "latest" ]]; then
    version="$(latest_version_under_tag "${env_root}" "${tag}" || true)"
    if [[ -z "${version}" ]]; then
      echo "error: no usable version under ${env_root}/targets/${tag}" >&2
      return 1
    fi
  fi
  local bundle="${env_root}/targets/${tag}/${version}"
  if [[ ! -d "${bundle}" && -d "${env_root}/targets/${tag}/mei-lang-${version}" ]]; then
    version="mei-lang-${version}"
    bundle="${env_root}/targets/${tag}/${version}"
  fi
  if [[ ! -x "${bundle}/v2/bin/mei-host-shell" && ! -x "${bundle}/bin/mei-host-shell" ]]; then
    echo "error: version not usable: ${bundle}" >&2
    echo "hint: mei.sh env list --env ${env_root} --tag ${tag}" >&2
    return 1
  fi
  if command -v jq >/dev/null 2>&1; then
    local tmp
    tmp="$(mktemp)"
    jq --arg tag "${tag}" --arg ver "${version}" --arg root "${env_root}" '
      .meiEnv = ((.meiEnv // {})
        | .root = $root
        | .targetTag = $tag
        | .version = $ver
        | del(.scenario))
    ' "${ws_json}" >"${tmp}" && mv "${tmp}" "${ws_json}"
  elif command -v python3 >/dev/null 2>&1; then
    python3 - "${ws_json}" "${tag}" "${version}" "${env_root}" <<'PY'
import json, sys
path, tag, ver, root = sys.argv[1:5]
with open(path, encoding="utf-8") as f:
    data = json.load(f)
mei = dict(data.get("meiEnv") or {})
mei.pop("scenario", None)
mei["root"] = root
mei["targetTag"] = tag
mei["version"] = ver
data["meiEnv"] = mei
with open(path, "w", encoding="utf-8") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
    f.write("\n")
PY
  else
    echo "error: need jq or python3 to pin meiEnv" >&2
    return 1
  fi
  echo "==> pinned meiEnv root=${env_root} tag=${tag} version=${version} → ${workspace_root}"
}

list_usable_versions() {
  local mei_env_root="$1"
  local tag="$2"
  local tag_root="${mei_env_root}/targets/${tag}"
  [[ -d "${tag_root}" ]] || return 0
  local d
  for d in "${tag_root}"/*; do
    [[ -d "${d}" ]] || continue
    if [[ -x "${d}/v2/bin/mei-host-shell" || -x "${d}/bin/mei-host-shell" ]]; then
      printf '%s\n' "$(basename "${d}")"
    fi
  done
}

print_mei_banner() {
  local cmd="$1"
  echo "── mei.sh ${cmd} ────────────────────────────────"
  echo "mei-lang:    ${SOURCE:-${MEI_LANG_ROOT_DEFAULT}}"
  echo "mei-env:     root=${ENV_ROOT_OPT:-?} tag=${TARGET_TAG:-?} version=${VERSION_PIN:-"(unspecified)"}"
  if [[ -n "${WORKSPACE}" ]]; then
    echo "workspace:   ${WORKSPACE}"
  fi
  echo "────────────────────────────────────────────────"
}

fill_from_source() {
  local source="$1"
  local env_root="$2"
  local tag="$3"
  local profile="$4"
  local fill_mono="${env_root}/release/collect/fill-from-lang.sh"
  local fill_lang="${ENV_DIR}/fill-lang-into-env.sh"

  if [[ ! -f "${source}/Cargo.toml" ]]; then
    echo "error: --source is not a mei-lang tree: ${source}" >&2
    return 1
  fi

  echo "==> fill from source=${source} → env=${env_root} tag=${tag} profile=${profile}"
  if [[ -x "${fill_mono}" ]]; then
    MEI_LANG_ROOT="${source}" MEI_ENV_ROOT="${env_root}" \
      "${fill_mono}" --tag "${tag}" --profile "${profile}"
  elif [[ -x "${fill_lang}" ]]; then
    MEI_LANG_ROOT="${source}" MEI_ENV_ROOT="${env_root}" \
      "${fill_lang}" --tag "${tag}" --profile "${profile}"
  else
    echo "error: no fill script (expected ${fill_mono} or ${fill_lang})" >&2
    return 1
  fi
}

fill_from_bundle() {
  local bundle="$1"
  local env_root="$2"
  local tag="$3"
  local fill_bundle="${ENV_DIR}/fill-bundle-into-env.sh"
  if [[ ! -e "${bundle}" ]]; then
    echo "error: --bundle not found: ${bundle}" >&2
    return 1
  fi
  local args=(--tag "${tag}" --from "${bundle}")
  if [[ -n "${VERSION_PIN}" && "${VERSION_PIN}" != "latest" ]]; then
    args+=(--version "${VERSION_PIN}")
  fi
  echo "==> fill from bundle=${bundle} → env=${env_root} tag=${tag}"
  MEI_ENV_ROOT="${env_root}" "${fill_bundle}" "${args[@]}"
}

# Shared option state
SOURCE=""
ENV_ROOT_OPT=""
BUNDLE=""
TARGET_TAG=""
PROFILE="debug"
WORKSPACE=""
WORKSPACE_ID=""
WORKSPACE_LABEL=""
APP_ID=""
NO_FILL=0
VERSION_PIN=""
LEGACY_SCENARIO=""

parse_common_args() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --source) SOURCE="${2:?}"; shift 2 ;;
      --source=*) SOURCE="${1#*=}"; shift ;;
      --env|--mei-env-root) ENV_ROOT_OPT="${2:?}"; shift 2 ;;
      --env=*|--mei-env-root=*) ENV_ROOT_OPT="${1#*=}"; shift ;;
      --bundle|--from-bundle) BUNDLE="${2:?}"; shift 2 ;;
      --bundle=*|--from-bundle=*) BUNDLE="${1#*=}"; shift ;;
      --mei-lang-root) SOURCE="${2:?}"; shift 2 ;; # alias
      --mei-lang-root=*) SOURCE="${1#*=}"; shift ;;
      --tag) TARGET_TAG="${2:?}"; shift 2 ;;
      --tag=*) TARGET_TAG="${1#*=}"; shift ;;
      --profile) PROFILE="${2:?}"; shift 2 ;;
      --profile=*) PROFILE="${1#*=}"; shift ;;
      --release) PROFILE="release"; shift ;;
      --debug) PROFILE="debug"; shift ;;
      --workspace) WORKSPACE="${2:?}"; shift 2 ;;
      --workspace=*) WORKSPACE="${1#*=}"; shift ;;
      --workspace-id|--id) WORKSPACE_ID="${2:?}"; shift 2 ;;
      --workspace-id=*|--id=*) WORKSPACE_ID="${1#*=}"; shift ;;
      --app) APP_ID="${2:?}"; shift 2 ;;
      --app=*) APP_ID="${1#*=}"; shift ;;
      --label) WORKSPACE_LABEL="${2:?}"; shift 2 ;;
      --label=*) WORKSPACE_LABEL="${1#*=}"; shift ;;
      --version) VERSION_PIN="${2:?}"; shift 2 ;;
      --version=*) VERSION_PIN="${1#*=}"; shift ;;
      --no-fill) NO_FILL=1; shift ;;
      --no-install|--copy) shift ;; # deprecated no-op
      --scenario)
        LEGACY_SCENARIO="${2:?}"
        echo "warn: --scenario is deprecated; use --source / --env / --bundle" >&2
        shift 2
        ;;
      --scenario=*)
        LEGACY_SCENARIO="${1#*=}"
        echo "warn: --scenario is deprecated; use --source / --env / --bundle" >&2
        shift
        ;;
      -h|--help) usage; exit 0 ;;
      *)
        echo "unknown arg: $1" >&2
        usage >&2
        exit 1
        ;;
    esac
  done
}

apply_legacy_scenario() {
  case "${LEGACY_SCENARIO}" in
    "") return 0 ;;
    monorepo)
      if [[ -z "${SOURCE}" ]]; then SOURCE="$(detect_default_source || true)"; fi
      if [[ -z "${ENV_ROOT_OPT}" ]]; then
        ENV_ROOT_OPT="$(detect_monorepo_env "${SOURCE}" || true)"
        if [[ -z "${ENV_ROOT_OPT}" ]]; then
          echo "error: --scenario monorepo but mei-env not found (pass --env)" >&2
          exit 1
        fi
      fi
      ;;
    lang-source)
      if [[ -z "${SOURCE}" ]]; then SOURCE="$(detect_default_source || true)"; fi
      if [[ -z "${ENV_ROOT_OPT}" ]]; then ENV_ROOT_OPT="${HOME}/.mei-env"; fi
      ;;
    installed)
      if [[ -z "${BUNDLE}" && -d "./bundle" ]]; then
        BUNDLE="$(cd ./bundle && pwd)"
      fi
      if [[ -z "${ENV_ROOT_OPT}" ]]; then ENV_ROOT_OPT="${HOME}/.mei-env"; fi
      ;;
    *)
      echo "error: unknown deprecated --scenario=${LEGACY_SCENARIO}" >&2
      exit 1
      ;;
  esac
}

resolve_paths() {
  apply_legacy_scenario

  if [[ -n "${BUNDLE}" && -n "${SOURCE}" ]]; then
    echo "error: --source and --bundle are mutually exclusive" >&2
    exit 1
  fi

  if [[ -z "${SOURCE}" && -z "${BUNDLE}" ]]; then
    SOURCE="$(detect_default_source || true)"
  fi

  local env_raw="${ENV_ROOT_OPT}"
  if [[ -z "${env_raw}" ]]; then
    if env_raw="$(detect_monorepo_env "${SOURCE:-}" 2>/dev/null)"; then
      :
    else
      env_raw="${HOME}/.mei-env"
    fi
  fi

  if [[ -n "${SOURCE}" ]]; then
    if [[ ! -d "${SOURCE}" ]]; then
      echo "error: --source not a directory: ${SOURCE}" >&2
      exit 1
    fi
    SOURCE="$(cd "${SOURCE}" && pwd)"
  fi

  if [[ -n "${BUNDLE}" && -e "${BUNDLE}" && -d "${BUNDLE}" ]]; then
    BUNDLE="$(cd "${BUNDLE}" && pwd)"
  fi

  mkdir -p "${env_raw}" 2>/dev/null || true
  ENV_ROOT_OPT="$(cd "${env_raw}" && pwd)"

  if [[ -z "${TARGET_TAG}" ]]; then
    TARGET_TAG="$(default_target_tag)"
  fi
}

cmd_init() {
  parse_common_args "$@"
  resolve_paths
  print_mei_banner "init"

  if [[ "${NO_FILL}" -eq 0 ]]; then
    if [[ -z "${BUNDLE}" && -z "${SOURCE}" ]]; then
      if [[ -d "./bundle" ]]; then
        BUNDLE="$(cd ./bundle && pwd)"
      else
        echo "error: init requires --source (mei-lang) or --bundle" >&2
        exit 1
      fi
    fi
  fi

  ENV_ROOT_OPT="$("${ENV_DIR}/ensure-mei-env-root.sh" --root "${ENV_ROOT_OPT}")"
  export MEI_ENV_ROOT="${ENV_ROOT_OPT}"
  [[ -n "${SOURCE}" ]] && export MEI_LANG_ROOT="${SOURCE}"

  if [[ "${NO_FILL}" -eq 0 ]]; then
    if [[ -n "${BUNDLE}" ]]; then
      fill_from_bundle "${BUNDLE}" "${ENV_ROOT_OPT}" "${TARGET_TAG}"
    else
      fill_from_source "${SOURCE}" "${ENV_ROOT_OPT}" "${TARGET_TAG}" "${PROFILE}"
    fi
  else
    echo "==> skip fill (--no-fill)"
  fi

  local resolved_ver="${VERSION_PIN}"
  if [[ -z "${resolved_ver}" || "${resolved_ver}" == "latest" ]]; then
    resolved_ver="$(latest_version_under_tag "${ENV_ROOT_OPT}" "${TARGET_TAG}" || true)"
  fi
  if [[ -n "${WORKSPACE}" ]]; then
    if [[ -z "${resolved_ver}" ]]; then
      echo "error: cannot resolve meiEnv.version for workspace (pass --version)" >&2
      echo "hint: mei.sh env list --env ${ENV_ROOT_OPT} --tag ${TARGET_TAG}" >&2
      exit 1
    fi
    if [[ "${NO_FILL}" -eq 1 && -z "${VERSION_PIN}" ]]; then
      echo "error: --no-fill with --workspace requires explicit --version" >&2
      echo "hint: mei.sh env list --env ${ENV_ROOT_OPT} --tag ${TARGET_TAG}" >&2
      exit 1
    fi
    VERSION_PIN="${resolved_ver}"
  fi

  if [[ -z "${WORKSPACE}" ]]; then
    echo "==> toolchain ready at ${ENV_ROOT_OPT}"
    if [[ -n "${resolved_ver}" ]]; then
      echo "    version=${resolved_ver}"
    fi
    echo "    next: mei.sh workspace init --dir <ws> --env ${ENV_ROOT_OPT} --version <id>"
    echo "       or: mei.sh env list --env ${ENV_ROOT_OPT}"
    exit 0
  fi

  mkdir -p "${WORKSPACE}"
  WORKSPACE="$(cd "${WORKSPACE}" && pwd)"
  [[ -z "${WORKSPACE_ID}" ]] && WORKSPACE_ID="$(basename "${WORKSPACE}")"

  local init_sh="${MEI_LANG_ROOT_DEFAULT}/stock/workspace/init-workspace.sh"
  if [[ -n "${SOURCE}" && -f "${SOURCE}/stock/workspace/init-workspace.sh" ]]; then
    init_sh="${SOURCE}/stock/workspace/init-workspace.sh"
  fi
  if [[ ! -f "${init_sh}" ]]; then
    echo "error: init-workspace.sh not found" >&2
    exit 1
  fi
  chmod +x "${init_sh}" 2>/dev/null || true

  local init_args=(--dir "${WORKSPACE}" --id "${WORKSPACE_ID}" \
    --mei-env-root "${ENV_ROOT_OPT}" --tag "${TARGET_TAG}" --version "${VERSION_PIN}")
  if [[ -n "${SOURCE}" ]]; then
    init_args+=(--mei-lang-root "${SOURCE}" --source "${SOURCE}")
  fi
  [[ -n "${APP_ID}" ]] && init_args+=(--app "${APP_ID}")
  [[ -n "${WORKSPACE_LABEL}" ]] && init_args+=(--label "${WORKSPACE_LABEL}")
  if [[ -n "${BUNDLE}" && -z "${SOURCE}" ]]; then
    init_args+=(--from-bundle)
  fi

  echo "==> init workspace ${WORKSPACE}"
  MEI_LANG_ROOT="${SOURCE:-${MEI_LANG_ROOT_DEFAULT}}" MEI_ENV_ROOT="${ENV_ROOT_OPT}" \
    "${init_sh}" "${init_args[@]}"

  echo "==> mei.sh init done"
  echo "    workspace=${WORKSPACE} meiEnv.version=${VERSION_PIN}"
  echo "    next: cd ${WORKSPACE} && ./deploy/build-app.sh && ./deploy/start-host.sh"
}

cmd_env_list() {
  parse_common_args "$@"
  local tag_user="${TARGET_TAG}"
  resolve_paths
  if [[ -z "${tag_user}" ]]; then
    TARGET_TAG=""
  fi
  print_mei_banner "env list"

  local targets="${ENV_ROOT_OPT}/targets"
  if [[ ! -d "${targets}" ]]; then
    echo "error: no targets/ under ${ENV_ROOT_OPT}" >&2
    exit 1
  fi

  if [[ -z "${TARGET_TAG}" ]]; then
    echo "tags under ${targets}:"
    local t newest
    for t in "${targets}"/*; do
      [[ -d "${t}" ]] || continue
      newest="$(latest_version_under_tag "${ENV_ROOT_OPT}" "$(basename "${t}")" || true)"
      echo "  $(basename "${t}")${newest:+  (latest=${newest})}"
    done
    echo "hint: mei.sh env list --env ${ENV_ROOT_OPT} --tag <tag>"
    return 0
  fi

  echo "versions under ${targets}/${TARGET_TAG}:"
  local ver newest
  newest="$(latest_version_under_tag "${ENV_ROOT_OPT}" "${TARGET_TAG}" || true)"
  while IFS= read -r ver; do
    [[ -n "${ver}" ]] || continue
    if [[ "${ver}" == "${newest}" ]]; then
      echo "  ${ver}  *"
    else
      echo "  ${ver}"
    fi
  done < <(list_usable_versions "${ENV_ROOT_OPT}" "${TARGET_TAG}" | sort)
  if [[ -z "${newest}" ]]; then
    echo "  (none usable — need v2/bin/mei-host-shell)"
  else
    echo "(* = latest by mtime)"
  fi
}

cmd_env_build() {
  parse_common_args "$@"
  resolve_paths
  print_mei_banner "env build"

  if [[ -z "${SOURCE}" ]]; then
    echo "error: env build requires --source (or run inside / beside mei-lang)" >&2
    exit 1
  fi
  if [[ -n "${BUNDLE}" ]]; then
    echo "error: env build is for source compile; use: mei.sh init --bundle …" >&2
    exit 1
  fi

  ENV_ROOT_OPT="$("${ENV_DIR}/ensure-mei-env-root.sh" --root "${ENV_ROOT_OPT}")"
  export MEI_ENV_ROOT="${ENV_ROOT_OPT}" MEI_LANG_ROOT="${SOURCE}"

  fill_from_source "${SOURCE}" "${ENV_ROOT_OPT}" "${TARGET_TAG}" "${PROFILE}"

  local version="${VERSION_PIN}"
  if [[ -z "${version}" || "${version}" == "latest" ]]; then
    version="$(latest_version_under_tag "${ENV_ROOT_OPT}" "${TARGET_TAG}" || true)"
  fi
  if [[ -n "${WORKSPACE}" ]]; then
    if [[ -z "${version}" ]]; then
      echo "error: fill produced no usable version to pin" >&2
      exit 1
    fi
    WORKSPACE="$(cd "${WORKSPACE}" && pwd)"
    pin_workspace_mei_env "${WORKSPACE}" "${ENV_ROOT_OPT}" "${TARGET_TAG}" "${version}"
  elif [[ -n "${version}" ]]; then
    echo "==> filled version=${version} (pass --workspace --version … to pin meiEnv)"
  fi

  echo "Next: cd <workspace> && ./deploy/build-app.sh && ./deploy/start-host.sh"
}

cmd_env_pin() {
  parse_common_args "$@"
  resolve_paths
  print_mei_banner "env pin"

  if [[ -z "${WORKSPACE}" ]]; then
    echo "error: env pin requires --workspace" >&2
    exit 1
  fi
  if [[ -z "${VERSION_PIN}" ]]; then
    echo "error: env pin requires --version <id|latest>" >&2
    echo "hint: mei.sh env list --env ${ENV_ROOT_OPT} --tag ${TARGET_TAG}" >&2
    exit 1
  fi
  WORKSPACE="$(cd "${WORKSPACE}" && pwd)"
  pin_workspace_mei_env "${WORKSPACE}" "${ENV_ROOT_OPT}" "${TARGET_TAG}" "${VERSION_PIN}"
}

cmd_workspace_init() {
  # Accept --dir as alias for --workspace
  local rewritten=()
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dir) rewritten+=(--workspace "$2"); shift 2 ;;
      --dir=*) rewritten+=(--workspace "${1#*=}"); shift ;;
      *) rewritten+=("$1"); shift ;;
    esac
  done
  parse_common_args "${rewritten[@]}"
  resolve_paths
  print_mei_banner "workspace init"

  if [[ -z "${VERSION_PIN}" ]]; then
    VERSION_PIN="$(latest_version_under_tag "${ENV_ROOT_OPT}" "${TARGET_TAG}" || true)"
  fi
  if [[ -z "${VERSION_PIN}" ]]; then
    echo "error: workspace init requires --version (no usable bundle found)" >&2
    echo "hint: mei.sh env list --env ${ENV_ROOT_OPT}" >&2
    exit 1
  fi

  local init_sh="${MEI_LANG_ROOT_DEFAULT}/stock/workspace/init-workspace.sh"
  if [[ -n "${SOURCE}" && -f "${SOURCE}/stock/workspace/init-workspace.sh" ]]; then
    init_sh="${SOURCE}/stock/workspace/init-workspace.sh"
  fi
  [[ -f "${init_sh}" ]] || { echo "error: init-workspace.sh missing" >&2; exit 1; }

  local dir="${WORKSPACE:-$(pwd)}"
  local args=(--dir "${dir}" --mei-env-root "${ENV_ROOT_OPT}" --tag "${TARGET_TAG}" --version "${VERSION_PIN}")
  [[ -n "${SOURCE}" ]] && args+=(--mei-lang-root "${SOURCE}" --source "${SOURCE}")
  [[ -n "${WORKSPACE_ID}" ]] && args+=(--id "${WORKSPACE_ID}")
  [[ -n "${APP_ID}" ]] && args+=(--app "${APP_ID}")
  [[ -n "${WORKSPACE_LABEL}" ]] && args+=(--label "${WORKSPACE_LABEL}")
  if [[ -z "${SOURCE}" ]]; then
    args+=(--from-bundle)
  fi

  MEI_LANG_ROOT="${SOURCE:-${MEI_LANG_ROOT_DEFAULT}}" MEI_ENV_ROOT="${ENV_ROOT_OPT}" \
    exec bash "${init_sh}" "${args[@]}"
}

# ── main ──────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  usage >&2
  exit 1
fi

CMD="$1"
shift

case "${CMD}" in
  init)
    cmd_init "$@"
    ;;
  env)
    SUB="${1:-}"
    if [[ -z "${SUB}" ]]; then
      echo "error: use: mei.sh env list|build|pin" >&2
      exit 1
    fi
    shift
    case "${SUB}" in
      list) cmd_env_list "$@" ;;
      build) cmd_env_build "$@" ;;
      pin) cmd_env_pin "$@" ;;
      *)
        echo "error: unknown env subcommand: ${SUB}" >&2
        exit 1
        ;;
    esac
    ;;
  workspace)
    SUB="${1:-}"
    if [[ "${SUB}" != "init" ]]; then
      echo "error: use: mei.sh workspace init …" >&2
      exit 1
    fi
    shift
    cmd_workspace_init "$@"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "error: unknown command: ${CMD}" >&2
    usage >&2
    exit 1
    ;;
esac
