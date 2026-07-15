#!/usr/bin/env python3
"""Split resource_tool_bridge.rs into core/executor/tests."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "server/src/resource_tool_bridge.rs"
OUT = ROOT / "server/src/resource_tool_bridge"


def main():
    lines = SRC.read_text(encoding="utf-8").splitlines(keepends=True)
    imp = "".join(lines[:20])
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "core.rs").write_text("".join(lines[:112]))
    (OUT / "executor.rs").write_text(
        imp + "use super::core::SceneResourceToolExecutor;\n\n" + "".join(lines[112:341])
    )
    (OUT / "tests.rs").write_text(
        imp + "use super::core::SceneResourceToolExecutor;\n\n" + "".join(lines[341:])
    )
    (OUT / "mod.rs").write_text(
        "mod core;\nmod executor;\n#[cfg(test)]\nmod tests;\n\npub use core::SceneResourceToolExecutor;\n"
    )
    SRC.unlink()
    print("done")


if __name__ == "__main__":
    main()
