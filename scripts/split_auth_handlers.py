#!/usr/bin/env python3
"""Split auth_api/handlers.rs with shared support module."""
from __future__ import annotations
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "server/src/http/auth_api/handlers.rs"
OUT = ROOT / "server/src/http/auth_api/handlers"


def sl(lines, a, b):
    return "".join(lines[a - 1 : b])


def promote_support(content: str) -> str:
    out = []
    for line in content.splitlines(keepends=True):
        s = line.lstrip()
        if s.startswith("fn ") or s.startswith("struct "):
            out.append(line.replace(s, f"pub(super) {s}", 1))
        else:
            out.append(line)
    return "".join(out)


def main():
    lines = SRC.read_text(encoding="utf-8").splitlines(keepends=True)
    imports = sl(lines, 1, 21)
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "support.rs").write_text(imports + promote_support(sl(lines, 22, 112)), encoding="utf-8")
    (OUT / "pages_api.rs").write_text(
        imports + "use super::support::*;\n\n" + sl(lines, 113, 212),
        encoding="utf-8",
    )
    (OUT / "session_api.rs").write_text(
        imports + "use super::support::*;\n\n" + sl(lines, 213, len(lines)),
        encoding="utf-8",
    )
    (OUT / "mod.rs").write_text(
        "mod support;\nmod pages_api;\nmod session_api;\n\npub use pages_api::*;\npub use session_api::*;\n",
        encoding="utf-8",
    )
    SRC.unlink()
    print("done")


if __name__ == "__main__":
    main()
