#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MEI_LANG_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

require_cmd() {
  local name="$1"
  command -v "$name" >/dev/null 2>&1 || {
    echo "error: missing command: $name" >&2
    exit 1
  }
}

require_cmd cargo
require_cmd python3

cd "${MEI_LANG_ROOT}"

echo "==> toolchain contract: capability catalog"
cargo test -p mei-lang-toolchain capability_catalog_includes_platform_assets_and_profiles

echo "==> toolchain contract: bounded dataset rows"
cargo test -p mei-lang-toolchain query_world_dataset_contract_shape_is_stable

echo "==> toolchain contract: bounded dataset metrics"
cargo test -p mei-lang-toolchain query_world_dataset_metrics_contract_shape_is_stable

echo "==> toolchain contract: access knowledge bundle"
cargo test -p mei-lang-toolchain knowledge_bundle_exports_access_assets

echo "==> access profile: browser query state merge"
cargo test -p mei-lang-server browser_query_state_merges_active_entries

echo "==> author profile: host prompt stays off old skill injection path"
cargo test -p mei-lang-server system_prompt_has_tool_policy_and_no_inlined_companion_bodies

echo "==> scope preview: context signature still tracks access coordinates"
cargo test -p mei-lang-server context_signature_tracks_scope_fields

echo "==> access runtime: host tool defs follow catalog"
cargo test -p mei-lang-server access_tools_follow_catalog_host_bound_names

echo "==> access runtime: trace export stays wired"
cargo test -p mei-lang-server resource_runtime_trace_export_ok_with_valid_snapshot_scope

echo "==> CLI smoke: catalog export"
CATALOG_JSON="$(cargo run -p mei-lang-server --bin mei-toolchain -- mcp catalog --json)"
CATALOG_JSON="${CATALOG_JSON}" python3 - <<'PY'
import json
import os
import sys

payload = json.loads(os.environ["CATALOG_JSON"])
assert payload["schema_version"] == "mei-capability-catalog-v1"
assert len(payload.get("ai_profiles", [])) >= 2
assert any(item.get("id") == "meilang-access" for item in payload.get("skill_packages", []))
assert any(
    item.get("id") == "access"
    and item.get("skill_package_id") == "meilang-access"
    for item in payload.get("ai_profiles", [])
)
platform_assets = payload.get("platform_assets", {})
assert len(platform_assets.get("component_packs", [])) > 0
assert any(item.get("id") == "cockpit" for item in platform_assets.get("template_packs", []))
print("catalog ok")
PY

echo "==> done: author/access golden smoke checks passed"
