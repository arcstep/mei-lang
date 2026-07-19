#!/usr/bin/env python3
"""Render Cargo JSON diagnostics and print a compact freshness summary."""

from __future__ import annotations

import json
import sys
import time


def artifact_label(message: dict[str, object]) -> str:
    target = message.get("target")
    if not isinstance(target, dict):
        return "unknown"
    name = str(target.get("name", "unknown"))
    kinds = target.get("kind")
    if isinstance(kinds, list) and kinds:
        return f"{name}[{','.join(str(kind) for kind in kinds)}]"
    return name


def main() -> int:
    started = time.monotonic()
    rebuilt: set[str] = set()
    reused: set[str] = set()
    build_success = True

    for raw_line in sys.stdin:
        try:
            message = json.loads(raw_line)
        except json.JSONDecodeError:
            sys.stdout.write(raw_line)
            continue

        reason = message.get("reason")
        if reason == "compiler-message":
            compiler_message = message.get("message")
            if isinstance(compiler_message, dict):
                rendered = compiler_message.get("rendered")
                if isinstance(rendered, str):
                    sys.stderr.write(rendered)
        elif reason == "compiler-artifact":
            label = artifact_label(message)
            if message.get("fresh") is True:
                reused.add(label)
            else:
                rebuilt.add(label)
                reused.discard(label)
        elif reason == "build-finished":
            build_success = message.get("success") is True

    elapsed = time.monotonic() - started
    rebuilt_targets = sorted(rebuilt)
    visible_targets = rebuilt_targets[:24]
    rebuilt_list = ", ".join(visible_targets) if visible_targets else "none"
    if len(rebuilt_targets) > len(visible_targets):
        rebuilt_list += f", ... (+{len(rebuilt_targets) - len(visible_targets)} more)"
    print(
        "==> cargo freshness: "
        f"rebuilt={len(rebuilt)} reused={len(reused)} elapsed={elapsed:.2f}s"
    )
    print(f"    rebuilt targets: {rebuilt_list}")
    return 0 if build_success else 1


if __name__ == "__main__":
    raise SystemExit(main())
