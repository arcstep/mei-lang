#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAX=501 FAIL=0
scan(){ local l="$1"; shift; while IFS= read -r f; do [[ -z "$f" ]] && continue; n=$(wc -l < "$f" | tr -d ' ');
  [[ "$n" -gt $MAX ]] && echo "FAIL $f: $n" >&2 && FAIL=1; done < <(find "$@" \( -path '*/target/*' -o -path '*/node_modules/*' -o -path '*/vendor/*' -o -path '*/dist/*' \) -prune -o \( -name '*.rs' -o -name '*.js' \) -type f -print 2>/dev/null); }
scan server "$ROOT/server"; scan crates "$ROOT/crates"; scan app-rs "$ROOT/app/src"
scan app-js "$ROOT/app/assets" "-not" "-path" "$ROOT/app/assets/vendor/*" "-not" "-path" "$ROOT/app/assets/dist/*"
[[ $FAIL -eq 0 ]] || exit 1; echo check-max-file-lines: OK
