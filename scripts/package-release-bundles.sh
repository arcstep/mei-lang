#!/usr/bin/env bash
# Build and package portable MeiLang runtime/toolchain bundles.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OUT_ROOT="${MEI_LANG_ROOT}/dist/bundles"
PRODUCT="all"
SKIP_BUILD=0
SKIP_ASSETS=0

usage() {
  cat <<'EOF'
Usage: package-release-bundles.sh [options]

Options:
  --product all|runtime|toolchain  Bundle selection (default: all)
  --out DIR                        Output directory
  --skip-build                     Reuse target/release binaries
  --skip-assets                    Reuse host-shell/app/assets/dist
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --product) PRODUCT="$2"; shift 2 ;;
    --product=*) PRODUCT="${1#*=}"; shift ;;
    --out) OUT_ROOT="$2"; shift 2 ;;
    --out=*) OUT_ROOT="${1#*=}"; shift ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --skip-assets) SKIP_ASSETS=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

case "${PRODUCT}" in
  all) PRODUCTS=(runtime toolchain) ;;
  runtime|toolchain) PRODUCTS=("${PRODUCT}") ;;
  *) echo "error: --product must be all, runtime, or toolchain" >&2; exit 2 ;;
esac

VERSION="$(node "${SCRIPT_DIR}/release/sync-versions.mjs" --print-version)"
TARGET="$(rustc -vV | sed -n 's/^host: //p' | head -1)"
if [[ -z "${TARGET}" ]]; then
  echo "error: could not determine Rust host target" >&2
  exit 1
fi

EXT=""
case "${TARGET}" in
  *windows*) EXT=".exe" ;;
esac

TARGET_DIR="${CARGO_TARGET_DIR:-${MEI_LANG_ROOT}/target}"
if [[ -n "${MEI_CARGO_TARGET_DIR:-}" ]]; then
  TARGET_DIR="${MEI_CARGO_TARGET_DIR}"
elif [[ "${TARGET_DIR}" != "${MEI_LANG_ROOT}/target" ]]; then
  unset CARGO_TARGET_DIR || true
  TARGET_DIR="${MEI_LANG_ROOT}/target"
fi
BIN_DIR="${TARGET_DIR}/release"

RUNTIME_BINS=(mei-host-shell mei-compiler mei-app-runtime mei-plug-ds)
TOOLCHAIN_BINS=(
  mei-toolchain
  mei-compiler
  mei-lsp
  mei-host-shell
  mei-app-runtime
  mei-plug-ds
  mei-snapshot
)
# External GIS tile server (not built from this workspace).
MARTIN_SIDECAR_BINS=(martin)

if [[ "${SKIP_BUILD}" != "1" ]]; then
  echo "==> building release binaries"
  CARGO_TARGET_DIR="${TARGET_DIR}" cargo build --release \
    --manifest-path "${MEI_LANG_ROOT}/Cargo.toml" \
    -p mei-host-shell \
    -p mei-compiler \
    -p mei-app-runtime \
    -p mei-plug-ds \
    -p mei-snapshot \
    -p mei-lang-lsp \
    -p mei-lang-server
fi

if [[ "${SKIP_ASSETS}" != "1" ]]; then
  echo "==> building host assets"
  (cd "${MEI_LANG_ROOT}" && npm run assets:build)
fi

copy_tree() {
  local source="$1"
  local destination="$2"
  python3 - "${source}" "${destination}" <<'PY'
import shutil
import sys
from pathlib import Path

source = Path(sys.argv[1])
destination = Path(sys.argv[2])
if not source.is_dir():
    raise SystemExit(f"missing resource directory: {source}")
if destination.exists():
    shutil.rmtree(destination)
shutil.copytree(source, destination, ignore=shutil.ignore_patterns("node_modules", ".git", ".DS_Store"))
PY
}

smoke_binary() {
  local binary="$1"
  local output
  output="$("${binary}" --version 2>&1)"
  if [[ "${output}" != *"${VERSION}"* ]]; then
    echo "error: ${binary} --version did not report ${VERSION}" >&2
    echo "${output}" >&2
    exit 1
  fi
  echo "  ok $(basename "${binary}"): ${output%%$'\n'*}"
}

write_internal_manifest() {
  local stage="$1"
  local product="$2"
  shift 2
  VERSION="${VERSION}" TARGET="${TARGET}" PRODUCT="${product}" \
    python3 - "${stage}" "$@" <<'PY'
import hashlib
import json
import os
import sys
from pathlib import Path

root = Path(sys.argv[1])
bins = sys.argv[2:]
files = []
for path in sorted(root.rglob("*")):
    if not path.is_file() or path.name == "MANIFEST.json":
        continue
    files.append({
        "path": path.relative_to(root).as_posix(),
        "bytes": path.stat().st_size,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    })
document = {
    "schemaVersion": 1,
    "product": os.environ["PRODUCT"],
    "version": os.environ["VERSION"],
    "target": os.environ["TARGET"],
    "bins": bins,
    "packageRoot": "share/mei",
    "files": files,
}
(root / "MANIFEST.json").write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
PY
}

write_archive_manifest() {
  local archive="$1"
  local product="$2"
  local internal_manifest="$3"
  python3 - "${archive}" "${product}" "${VERSION}" "${TARGET}" "${internal_manifest}" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

archive, product, version, target, internal_manifest = sys.argv[1:]
path = Path(archive)
internal = json.loads(Path(internal_manifest).read_text(encoding="utf-8"))
document = {
    "schemaVersion": 1,
    "product": product,
    "version": version,
    "target": target,
    "archive": path.name,
    "bytes": path.stat().st_size,
    "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    "bins": internal["bins"],
    "packageRoot": internal["packageRoot"],
}
out = path.with_name(path.name.removesuffix(".tar.gz").removesuffix(".zip") + ".manifest.json")
out.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
print(f"wrote {out}")
PY
}

verify_archive_contents() {
  local archive="$1"
  python3 - "${archive}" <<'PY'
import hashlib
import io
import json
import sys
import tarfile
import zipfile
from pathlib import Path

archive = Path(sys.argv[1])
if archive.name.endswith(".tar.gz"):
    with tarfile.open(archive, "r:gz") as source:
        names = {member.name for member in source.getmembers() if member.isfile()}
        manifest_name = next(name for name in names if name.endswith("/MANIFEST.json"))
        manifest = json.load(io.TextIOWrapper(source.extractfile(manifest_name), encoding="utf-8"))
        root = manifest_name.removesuffix("/MANIFEST.json")
        expected_names = {manifest_name}
        for entry in manifest["files"]:
            name = f"{root}/{entry['path']}"
            expected_names.add(name)
            if name not in names:
                raise SystemExit(f"{archive}: internal manifest entry is missing: {name}")
            actual = hashlib.sha256(source.extractfile(name).read()).hexdigest()
            if actual != entry["sha256"]:
                raise SystemExit(f"{archive}: SHA-256 mismatch for {name}")
        if names != expected_names:
            raise SystemExit(f"{archive}: files are not fully covered by internal manifest")
else:
    with zipfile.ZipFile(archive) as source:
        names = set(source.namelist())
        manifest_name = next(name for name in names if name.endswith("/MANIFEST.json"))
        manifest = json.loads(source.read(manifest_name))
        root = manifest_name.removesuffix("/MANIFEST.json")
        expected_names = {manifest_name}
        for entry in manifest["files"]:
            name = f"{root}/{entry['path']}"
            expected_names.add(name)
            if name not in names:
                raise SystemExit(f"{archive}: internal manifest entry is missing: {name}")
            actual = hashlib.sha256(source.read(name)).hexdigest()
            if actual != entry["sha256"]:
                raise SystemExit(f"{archive}: SHA-256 mismatch for {name}")
        if names != expected_names:
            raise SystemExit(f"{archive}: files are not fully covered by internal manifest")
print(f"verified archive contents: {archive.name}")
PY
}

package_product() {
  local product="$1"
  local -a bins
  if [[ "${product}" == "runtime" ]]; then
    bins=("${RUNTIME_BINS[@]}")
  else
    bins=("${TOOLCHAIN_BINS[@]}")
  fi

  local archive_base="mei-${product}-${VERSION}-${TARGET}"
  local stage="${OUT_ROOT}/stage/${archive_base}"
  rm -rf "${stage}"
  mkdir -p "${stage}/bin" "${stage}/share/mei/app"

  local name source destination
  for name in "${bins[@]}"; do
    source="${BIN_DIR}/${name}${EXT}"
    destination="${stage}/bin/${name}${EXT}"
    if [[ ! -f "${source}" ]]; then
      echo "error: missing release binary: ${source}" >&2
      exit 1
    fi
    cp -f "${source}" "${destination}"
    if [[ "$(uname -s)" == "Darwin" ]]; then
      codesign --force --sign - "${destination}" >/dev/null
    fi
    smoke_binary "${destination}"
  done

  echo "==> fetching Martin sidecar binary"
  "${SCRIPT_DIR}/fetch-martin-sidecar.sh" --dest "${stage}/bin"
  if [[ ! -f "${stage}/bin/martin" && ! -f "${stage}/bin/martin.exe" ]]; then
    echo "error: martin binary missing after fetch-martin-sidecar.sh" >&2
    exit 1
  fi
  local -a manifest_bins=("${bins[@]}" "${MARTIN_SIDECAR_BINS[@]}")

  copy_tree "${MEI_LANG_ROOT}/host-shell/app/assets" "${stage}/share/mei/app/assets"
  copy_tree "${MEI_LANG_ROOT}/stock" "${stage}/share/mei/stock"
  cp -f "${MEI_LANG_ROOT}/LICENSE" "${stage}/share/mei/LICENSE"
  write_internal_manifest "${stage}" "${product}" "${manifest_bins[@]}"

  mkdir -p "${OUT_ROOT}"
  local archive
  if [[ -n "${EXT}" ]]; then
    archive="${OUT_ROOT}/${archive_base}.zip"
    rm -f "${archive}"
    python3 - "${archive}" "${stage}" <<'PY'
import sys
import zipfile
from pathlib import Path

archive = Path(sys.argv[1])
root = Path(sys.argv[2])
with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
    for path in sorted(root.rglob("*")):
        if path.is_file():
            output.write(path, path.relative_to(root.parent).as_posix())
PY
  else
    archive="${OUT_ROOT}/${archive_base}.tar.gz"
    rm -f "${archive}"
    COPYFILE_DISABLE=1 tar -C "${OUT_ROOT}/stage" -czf "${archive}" "${archive_base}"
  fi

  verify_archive_contents "${archive}"
  write_archive_manifest "${archive}" "${product}" "${stage}/MANIFEST.json"
  echo "==> packaged ${archive}"
}

rm -rf "${OUT_ROOT}/stage"
for product in "${PRODUCTS[@]}"; do
  package_product "${product}"
done
rm -rf "${OUT_ROOT}/stage"
