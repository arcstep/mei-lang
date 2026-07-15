#!/usr/bin/env python3
"""Split server files at explicit line boundaries (function-safe)."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

JOBS = [
    (
        "server/src/resource_tool_bridge.rs",
        "server/src/resource_tool_bridge",
        [(1, 112), (113, 9999)],
        False,
    ),
    (
        "server/src/http/build_api/context_export.rs",
        "server/src/http/build_api/context_export",
        [(1, 240), (241, 9999)],
        True,
    ),
    (
        "server/src/http/auth_api/handlers.rs",
        "server/src/http/auth_api/handlers",
        [(1, 212), (213, 9999)],
        True,
    ),
    (
        "server/src/http/pages/dataset_api.rs",
        "server/src/http/pages/dataset_api",
        [(1, 463), (464, 9999)],
        True,
    ),
]


def import_prefix(lines: list[str]) -> list[str]:
    i = 0
    while i < len(lines):
        stripped = lines[i].strip()
        if lines[i].startswith("use ") or lines[i].startswith("pub use "):
            i += 1
            while i < len(lines) and (
                lines[i].startswith("    ")
                or lines[i].strip() in ("{", "};")
                or (lines[i].strip().endswith(",") and not lines[i].strip().startswith("fn "))
            ):
                i += 1
            continue
        if stripped == "" and i > 0:
            i += 1
            continue
        break
    return lines[:i]


def split_at(src: Path, out: Path, ranges: list[tuple[int, int]], pub: bool) -> None:
    lines = src.read_text(encoding="utf-8").splitlines(keepends=True)
    prefix = import_prefix(lines)
    out.mkdir(parents=True, exist_ok=True)
    names = ["a", "b"]
    vis = "pub use" if pub else "pub(crate) use"
    mods = []
    for (start, end), name in zip(ranges, names):
        body = lines[start - 1 : min(end, len(lines))]
        chunk = body if start == 1 else prefix + body
        (out / f"{name}.rs").write_text("".join(chunk), encoding="utf-8")
        mods.append(f"mod {name};")
        print(f"  {out.name}/{name}.rs: {len(chunk)}")
    (out / "mod.rs").write_text(
        "\n".join(mods) + f"\n\n{vis} a::*;\n{vis} b::*;\n",
        encoding="utf-8",
    )
    src.unlink()


def main() -> None:
    for rel, out_rel, ranges, pub in JOBS:
        src = ROOT / rel
        if not src.is_file():
            continue
        split_at(src, ROOT / out_rel, ranges, pub)
    print("done")


if __name__ == "__main__":
    main()
