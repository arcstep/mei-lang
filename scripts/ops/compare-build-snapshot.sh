#!/usr/bin/env bash
# Compare current target/ against a saved build snapshot baseline.
# Usage:
#   ./scripts/compare-build-snapshot.sh
#   ./scripts/compare-build-snapshot.sh /path/to/.build-snapshot-baseline-files.txt
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BASELINE_MANIFEST="${1:-${MEI_LANG_ROOT}/.build-snapshot-baseline-files.txt}"
TARGET_DIR="${CARGO_TARGET_DIR:-${MEI_LANG_ROOT}/target}"

if [[ ! -f "${BASELINE_MANIFEST}" ]]; then
  echo "error: baseline manifest not found: ${BASELINE_MANIFEST}" >&2
  exit 1
fi
if [[ ! -d "${TARGET_DIR}" ]]; then
  echo "error: target dir not found: ${TARGET_DIR}" >&2
  exit 1
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

current="${tmpdir}/current.txt"
{
  echo "# path	size_bytes	mtime_epoch"
  find "${TARGET_DIR}" -type f -print0 2>/dev/null | while IFS= read -r -d '' f; do
    stat -f "%N	%z	%m" "$f" 2>/dev/null | sed "s|^${TARGET_DIR}/||"
  done
} > "${current}"

echo "==> Disk usage (now)"
du -sh "${TARGET_DIR}" "${TARGET_DIR}/debug/deps" "${TARGET_DIR}/debug/incremental" 2>/dev/null || true
echo

baseline_count="$(tail -n +2 "${BASELINE_MANIFEST}" | wc -l | tr -d ' ')"
current_count="$(tail -n +2 "${current}" | wc -l | tr -d ' ')"
echo "==> File counts: baseline=${baseline_count}, now=${current_count}, delta=$((current_count - baseline_count))"
echo

echo "==> New files (in target, not in baseline)"
comm -13 <(tail -n +2 "${BASELINE_MANIFEST}" | cut -f1 | sort) <(tail -n +2 "${current}" | cut -f1 | sort) | head -200
new_total="$(comm -13 <(tail -n +2 "${BASELINE_MANIFEST}" | cut -f1 | sort) <(tail -n +2 "${current}" | cut -f1 | sort) | wc -l | tr -d ' ')"
if (( new_total > 200 )); then
  echo "... and $((new_total - 200)) more"
fi
echo

echo "==> New files total bytes"
python3 - "${BASELINE_MANIFEST}" "${current}" <<'PY'
import sys
from pathlib import Path

def load(path):
    rows = {}
    for line in Path(path).read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        rel, size, _mtime = line.split("\t", 2)
        rows[rel] = int(size)
    return rows

base = load(sys.argv[1])
cur = load(sys.argv[2])
new_paths = sorted(set(cur) - set(base))
removed = sorted(set(base) - set(cur))
changed = sorted(p for p in set(base) & set(cur) if base[p] != cur[p])

new_bytes = sum(cur[p] for p in new_paths)
removed_bytes = sum(base[p] for p in removed)
changed_growth = sum(max(0, cur[p] - base[p]) for p in changed)

print(f"new files: {len(new_paths)} ({new_bytes / (1024**3):.2f} GiB)")
print(f"removed files: {len(removed)} ({removed_bytes / (1024**3):.2f} GiB)")
print(f"size growth on existing paths: {changed_growth / (1024**3):.2f} GiB")
print(f"net file delta bytes: {(new_bytes - removed_bytes + changed_growth) / (1024**3):.2f} GiB")

print("\n==> Largest new files")
for p in sorted(new_paths, key=lambda x: cur[x], reverse=True)[:30]:
    print(f"{cur[p] / (1024**2):8.1f} MiB  {p}")
PY
