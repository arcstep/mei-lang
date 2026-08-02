#!/usr/bin/env bash
# Fail if mei-lang (public package) reintroduces business app defaults or sibling workspace paths.
# See .cursor/rules/lang-repo-boundary.mdc §4.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

EXCLUDE=(
  --glob '!**/.git/**'
  --glob '!**/target/**'
  --glob '!**/node_modules/**'
  --glob '!**/env/**'
  --glob '!**/var/**'
  --glob '!**/dist/**'
  --glob '!**/check-no-business-coupling.sh'
  --glob '!**/resolve-app.mjs'
  # In-repo fixtures / historical sample names (not live-host defaults)
  --glob '!**/tests/fixtures/**'
  --glob '!**/scripts/check/check-stage-architecture-schema-inventory.mjs'
  --glob '!**/0324-zhifa-layout-tuning-case-studies.md'
  --glob '!**/0328-zhifa-fill-down*'
)

fail=0

echo "==> [1/3] no sibling workspaces paths / absolute monorepo workspaces"
hits="$(rg -n --no-heading \
  -e 'workspaces/ws-spbjw' \
  -e 'workspaces/ws-thunder' \
  -e 'workspaces/ws-demos' \
  -e 'workspaces/ws-demo' \
  -e '\.\./workspaces/' \
  -e '/Users/[^[:space:]]*/workspaces/' \
  "${EXCLUDE[@]}" \
  scripts stock package.json .github 2>/dev/null || true)"
if [[ -n "$(echo "${hits}" | sed '/^$/d')" ]]; then
  echo "FAIL: sibling / absolute workspaces coupling:"
  echo "${hits}"
  fail=1
else
  echo "    OK"
fi

echo "==> [2/3] no business probe/perf npm scripts or script filenames"
hits="$(rg -n --no-heading \
  -e 'perf:zhifa' \
  -e 'probe_zhifa' \
  -e 'zhifa-runtime-perf' \
  "${EXCLUDE[@]}" \
  scripts package.json 2>/dev/null || true)"
# Allow comment-only mentions in README that say "forbidden" — still fail on package.json / code
if [[ -n "$(echo "${hits}" | sed '/^$/d')" ]]; then
  echo "FAIL: business probe/perf still in mei-lang:"
  echo "${hits}"
  fail=1
else
  echo "    OK"
fi

echo "==> [3/3] no hard-coded live-host /apps/zhifa (or qunfu) defaults in scripts"
hits="$(rg -n --no-heading \
  -e '"/apps/zhifa' \
  -e "'/apps/zhifa" \
  -e '`/apps/zhifa' \
  -e '/apps/zhifa/' \
  -e 'app=zhifa' \
  -e 'app_id=zhifa' \
  -e 'appId=zhifa' \
  -e '"/apps/qunfu' \
  -e '/apps/qunfu/' \
  -e '\|\| "zhifa"' \
  -e "\|\| 'zhifa'" \
  "${EXCLUDE[@]}" \
  scripts 2>/dev/null || true)"
if [[ -n "$(echo "${hits}" | sed '/^$/d')" ]]; then
  echo "FAIL: hard-coded business app live-host defaults:"
  echo "${hits}"
  fail=1
else
  echo "    OK"
fi

if [[ "${fail}" -ne 0 ]]; then
  echo "check-no-business-coupling: FAIL"
  exit 1
fi
echo "check-no-business-coupling: OK"
