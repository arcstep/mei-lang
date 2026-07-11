#!/usr/bin/env bash
# Fail if deleted/renamed layout authoring tokens reappear in sample apps / stock / guides.
# Kernel tests & ARCHIVED docs are out of scope for this gate (compiler policy rejects them).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

# Live call-sites / assignments (not ban tables)
LIVE='(frame\.add_panel\(|titled_shell\(|board_assembly\(|assembly_view\(|panel_slot\(|panel_contract\(|ops\.layoutTuning|row_budgets\s*=)'

ZERO_TOLERANCE=(
  workspaces/ws-demo-v2/apps/pretty-panels
  workspaces/ws-demo-v2/apps/mini-data
  workspaces/ws-demo-v2/stock/templates
)

DOC_GUIDE=(
  mei-lang/guides
  docs/mei-lang-v2
)

EXCLUDE_COMMON=(
  --glob '!**/.git/**'
  --glob '!**/target/**'
  --glob '!**/node_modules/**'
  --glob '!**/env/**'
  --glob '!**/var/**'
  --glob '!**/dist/**'
  --glob '!**/archive/**'
  --glob '!**/check-layout-legacy-blacklist.sh'
  --glob '!**/0324-pretty-panels-layout-tuning-case-studies.md'
  --glob '!**/0533-layout-policy-migration.md'
  --glob '!**/02100601-v1-equivalence.md'
  --glob '!**/021008-v1-v2-hybrid-authoring.md'
)

echo "==> [1/2] zero-tolerance: pretty-panels / mini-data / cockpit stock"
zt_hits="$(rg -n --no-heading -e "${LIVE}" "${EXCLUDE_COMMON[@]}" "${ZERO_TOLERANCE[@]}" 2>/dev/null || true)"
# Allow ban-commentary lines in stock README / macros headers
zt_hits="$(echo "${zt_hits}" | grep -v -E '禁止|已删除|DELETE|不得|不再|原 `micro_panel`|无 row_budgets|禁止 row_budgets|禁止 `row_budgets`' || true)"
if [[ -n "$(echo "${zt_hits}" | sed '/^$/d')" ]]; then
  echo "FAIL: banned live tokens in sample/stock:"
  echo "${zt_hits}"
  exit 1
fi
echo "    OK"

echo "==> [2/2] guides + v2 docs: no recommended live call-sites"
# Flag code fences / examples that still call deleted constructors (heuristic: line with call paren)
doc_hits="$(rg -n --no-heading -e "${LIVE}" "${EXCLUDE_COMMON[@]}" "${DOC_GUIDE[@]}" 2>/dev/null || true)"
doc_hits="$(echo "${doc_hits}" | grep -v -E '禁止|已删除|DELETE|RENAME|不得|不再|forbidden|Banned|blacklist|防回归|用 page_instance|用 content_panel|不要写|当前不要|已重命名|已废弃|ARCHIVED|@deprecated|迁移|对照|旧写法|替代' || true)"
# namespace-reference "当前不要写" block still lists tokens — exclude that file section by filtering bare listings without assignment context already in LIVE; keep failing real examples
doc_hits="$(echo "${doc_hits}" | grep -v 'namespace-reference.md' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v 'SKILL.md:' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v 'syntax-rules.md:' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v 'dsl-reference.md:' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v 'authoring.md:' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v 'components-reference.md:' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v '0300-ui-layout' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v '0327-t1-fill-down' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v '0328-pretty-panels-fill-down' || true)"
doc_hits="$(echo "${doc_hits}" | grep -v '0329-cockpit-viewport' || true)"
if [[ -n "$(echo "${doc_hits}" | sed '/^$/d')" ]]; then
  echo "FAIL: docs/guides still contain live banned call-sites:"
  echo "${doc_hits}"
  exit 1
fi
echo "    OK"

echo "layout legacy blacklist: OK"
