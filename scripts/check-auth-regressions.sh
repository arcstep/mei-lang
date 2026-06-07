#!/usr/bin/env bash
set -euo pipefail

echo "[auth] build mei-lang-server"
cargo build -p mei-lang-server 2>&1

echo "[auth] run auth regression tests"
cargo test -p mei-lang-server auth::tests 2>&1

echo "[auth] done"
