#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

echo "==> [1/5] host coordinates parity"
node ./scripts/test-agent-host-coordinates.mjs

echo "==> [2/5] editor MCP adapter smoke"
node ./scripts/test-editor-mcp-adapter.mjs

echo "==> [3/5] host contract preview digest regression"
cargo test -p mei-lang-server context_preview_echoes_host_protocol_and_affects_scope_digest -- --nocapture

echo "==> [4/5] publish-only redirect regression"
cargo test -p mei-lang-server access_only_surface_redirects_build_route_to_access_scene -- --nocapture

echo "==> [5/6] publish-only entrypoint regression"
cargo test -p mei-lang-server index_redirects_to_access_only_entry_when_surface_enabled -- --nocapture

echo "==> [6/6] theme token consumer lint"
node ./scripts/check-theme-tokens.mjs

echo "phase4 regressions passed"
