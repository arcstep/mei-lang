#!/usr/bin/env bash
# Probe how many width-copy concat arms / nest layers cause stack overflow.
# Each attempt runs the already-built test binary in a fresh OS process.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PHASE="${MEI_WIDTH_PROBE_PHASE:-both}"
INNER="${MEI_WIDTH_PROBE_INNER:-2}"
NEST="${MEI_WIDTH_PROBE_NEST:-1}"
STACK_KB="${MEI_WIDTH_PROBE_STACK_KB:-}" # empty = default thread stack
MIN_ARMS="${1:-1}"
MAX_ARMS="${2:-48}"
TEST="query_engine::pipeline_sql::controlled_sql_tdd_tests::width_copy_crash_probe_child"

echo "== compile probe test =="
COMPILE_LOG="$(mktemp)"
cargo test -p mei-lang-datasets width_copy_crash_probe_child --no-run >"$COMPILE_LOG" 2>&1
tail -5 "$COMPILE_LOG"
BIN="$(rg -o '/[^ ]+/mei_lang_datasets-[a-f0-9]+' "$COMPILE_LOG" | tail -1 || true)"
rm -f "$COMPILE_LOG"
if [[ -z "${BIN}" || ! -x "${BIN}" ]]; then
  echo "ERROR: could not locate mei_lang_datasets test binary" >&2
  exit 1
fi
echo "BIN=$BIN"

run_one() {
  local arms="$1"
  local log; log="$(mktemp)"
  set +e
  env MEI_WIDTH_PROBE_ARMS="$arms" \
      MEI_WIDTH_PROBE_INNER="$INNER" \
      MEI_WIDTH_PROBE_NEST="$NEST" \
      MEI_WIDTH_PROBE_PHASE="$PHASE" \
      ${STACK_KB:+MEI_WIDTH_PROBE_STACK_KB="$STACK_KB"} \
    "$BIN" "$TEST" --exact --include-ignored --nocapture \
    >"$log" 2>&1
  local rc=$?
  set -e
  rg -n "^PROBE |overflowed its stack|fatal runtime error" "$log" || true
  if rg -q "overflowed its stack|fatal runtime error: stack overflow" "$log"; then
    echo "RESULT arms=$arms status=STACK_OVERFLOW rc=$rc"
    rm -f "$log"
    return 134
  fi
  if [[ "$rc" -eq 0 ]] && rg -q "test result: ok" "$log"; then
    echo "RESULT arms=$arms status=OK rc=$rc"
    rm -f "$log"
    return 0
  fi
  echo "RESULT arms=$arms status=FAIL rc=$rc"
  tail -n 25 "$log" || true
  rm -f "$log"
  return "$rc"
}

echo "== probe phase=$PHASE inner=$INNER nest=$NEST stack_kb=${STACK_KB:-default} arms=[$MIN_ARMS..$MAX_ARMS] =="

last_ok=0
first_crash=""
for arms in $(seq "$MIN_ARMS" "$MAX_ARMS"); do
  if run_one "$arms"; then
    last_ok="$arms"
  else
    rc=$?
    if [[ "$rc" -eq 134 ]]; then
      first_crash="$arms"
      break
    fi
    echo "NOTE non-abort failure at arms=$arms rc=$rc"
  fi
done

echo "== SUMMARY =="
echo "last_ok_arms=$last_ok"
if [[ -n "$first_crash" ]]; then
  echo "first_stack_overflow_arms=$first_crash"
else
  echo "first_stack_overflow_arms=NONE_IN_RANGE"
fi
