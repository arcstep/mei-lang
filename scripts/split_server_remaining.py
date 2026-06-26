#!/usr/bin/env python3
"""Split remaining server oversized files and prebuild/tests.rs."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read_lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8").splitlines(keepends=True)


def sl(lines: list[str], start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    n = content.count("\n") + (0 if content.endswith("\n") else 1)
    print(f"  {'WARN' if n > 501 else 'OK'} {path.relative_to(ROOT)}: {n}")


def split_by_lines(src: Path, out_dir: Path, parts: list[tuple[str, int, int]], header: str = "") -> None:
    if not src.is_file():
        return
    lines = read_lines(src)
    mods = []
    for name, start, end in parts:
        body = header + sl(lines, start, end)
        write(out_dir / f"{name}.rs", body)
        mods.append(f"mod {name};")
    write(out_dir / "mod.rs", "\n".join(mods) + "\n")
    src.unlink()


def split_tests_by_test_fn(src: Path, out_dir: Path, header: str) -> None:
    if not src.is_file():
        return
    lines = read_lines(src)
    idx = next((i for i, l in enumerate(lines) if l.strip() == "mod tests {"), None)
    if idx is None:
        return
    write(out_dir / "mod.rs", sl(lines, 1, idx) + "\n#[cfg(test)]\nmod cases;\n")
    body = "".join(lines[idx + 1 : -1])
    chunks: list[str] = []
    cur: list[str] = []
    for line in body.splitlines(keepends=True):
        if line.lstrip().startswith("#[test]") and cur:
            chunks.append("".join(cur))
            cur = []
        cur.append(line)
    if cur:
        chunks.append("".join(cur))
    per = 5
    case_mods = ["use super::*;", header]
    for i in range(0, len(chunks), per):
        name = f"c{i // per + 1}"
        write(out_dir / f"{name}.rs", header + "\n" + "".join(chunks[i : i + per]))
        case_mods.append(f"mod {name};")
    write(out_dir / "cases.rs", "\n".join(case_mods) + "\n")
    src.unlink()


def main() -> None:
    split_by_lines(
        ROOT / "server/src/resource_tool_bridge.rs",
        ROOT / "server/src/resource_tool_bridge",
        [("types", 1, 200), ("executor", 201, 400), ("handlers", 401, 604)],
    )
    split_by_lines(
        ROOT / "server/src/http/build_api/context_export.rs",
        ROOT / "server/src/http/build_api/context_export",
        [("types", 1, 180), ("export", 181, 360), ("helpers", 361, 538)],
    )
    split_by_lines(
        ROOT / "server/src/http/auth_api/handlers.rs",
        ROOT / "server/src/http/auth_api/handlers",
        [("session", 1, 180), ("login", 181, 360), ("misc", 361, 539)],
    )
    split_by_lines(
        ROOT / "server/src/http/pages/dataset_api.rs",
        ROOT / "server/src/http/pages/dataset_api",
        [("types", 1, 170), ("query", 171, 400), ("recompute", 401, 657)],
    )
    # app/page/mod.rs — keep mod.rs as facade only
    src = ROOT / "server/src/http/pages/app/page/mod.rs"
    if src.is_file() and len(read_lines(src)) > 501:
        lines = read_lines(src)
        split_at = 280
        write(ROOT / "server/src/http/pages/app/page/render.rs", sl(lines, 1, 30) + sl(lines, 31, split_at))
        write(ROOT / "server/src/http/pages/app/page/access.rs", sl(lines, 1, 30) + sl(lines, split_at + 1, len(lines)))
        write(
            ROOT / "server/src/http/pages/app/page/mod.rs",
            sl(lines, 1, 30)
            + "mod render;\nmod access;\n\npub(crate) use render::*;\npub(crate) use access::*;\n",
        )

    split_tests_by_test_fn(
        ROOT / "server/src/prebuild/tests.rs",
        ROOT / "server/src/prebuild/tests",
        "use super::*;\n",
    )
    print("done")


if __name__ == "__main__":
    main()
