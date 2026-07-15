#!/usr/bin/env python3
"""Phase 7 production splits: toolchain knowledge_bundle/snapshot + app preview/nodes + topbar/view."""
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read_lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8").splitlines(keepends=True)


def sl(lines: list[str], start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    n = content.count("\n")
    print(f"  {'WARN' if n > 501 else 'OK'} {path.relative_to(ROOT)}: {n}")


def promote_pub_super(content: str) -> str:
    out: list[str] = []
    for line in content.splitlines(keepends=True):
        stripped = line.lstrip()
        indent = line[: len(line) - len(stripped)]
        if not stripped.startswith("pub") and stripped.startswith(("fn ", "struct ", "const ", "enum ")):
            out.append(f"{indent}pub(super) {stripped}")
        else:
            out.append(line)
    return "".join(out)


def split_file_module(
    src_rel: str,
    out_name: str,
    header_end: int,
    sections: list[tuple[str, int, int]],
    exports: str,
    extra_imports: dict[str, str] | None = None,
) -> None:
    src = ROOT / src_rel
    if not src.exists() or len(read_lines(src)) <= 501:
        print(f"SKIP {src_rel}")
        return
    lines = read_lines(src)
    out = ROOT / str(Path(src_rel).parent / out_name)
    header = sl(lines, 1, header_end)
    for name, start, end in sections:
        imp = extra_imports.get(name, "") if extra_imports else ""
        body = promote_pub_super(sl(lines, start, end))
        write(out / f"{name}.rs", header + imp + "\n" + body)
    write(out / "mod.rs", exports)
    src.unlink()
    print(f"split {src_rel} -> {out_name}/")


def split_knowledge_bundle() -> None:
    split_file_module(
        "crates/toolchain/src/knowledge_bundle.rs",
        "knowledge_bundle",
        53,
        [
            ("types", 8, 76),
            ("seeds_author", 77, 451),
            ("seeds_access", 452, 486),
            ("export", 487, 657),
        ],
        """mod types;
mod seeds_author;
mod seeds_access;
mod export;

pub use types::*;
pub use export::{
    export_knowledge_bundle_for_package_root, export_knowledge_bundle_for_workspace_root,
    knowledge_bundle_descriptor_for_package_root,
};
""",
        {
            "seeds_author": "use super::types::*;\n",
            "seeds_access": "use super::types::*;\n",
            "export": "use super::types::*;\nuse super::seeds_author::author_assets;\nuse super::seeds_access::access_assets;\n",
        },
    )


def split_world_snapshot() -> None:
    split_file_module(
        "crates/toolchain/src/world/snapshot.rs",
        "snapshot",
        30,
        [
            ("catalog_lines", 31, 122),
            ("helpers", 123, 156),
            ("business_summary", 157, 473),
            ("context_snapshot", 474, 559),
        ],
        """mod catalog_lines;
mod helpers;
mod business_summary;
mod context_snapshot;

pub use business_summary::build_world_business_summary;
pub use context_snapshot::build_world_context_snapshot;
""",
        {
            "helpers": "use super::catalog_lines::*;\n",
            "business_summary": "use super::catalog_lines::*;\nuse super::helpers::*;\n",
            "context_snapshot": "use super::catalog_lines::*;\nuse super::helpers::*;\nuse super::business_summary::*;\n",
        },
    )
    mod_path = ROOT / "crates/toolchain/src/world/mod.rs"
    if mod_path.exists():
        text = mod_path.read_text(encoding="utf-8")
        if "mod snapshot;" in text and "mod snapshot/" not in text:
            mod_path.write_text(text.replace("mod snapshot;", "mod snapshot;"), encoding="utf-8")


def split_preview_nodes() -> None:
    split_file_module(
        "app/src/ui/preview/nodes.rs",
        "nodes",
        27,
        [
            ("panel", 28, 279),
            ("dispatch", 280, 356),
            ("block", 357, 405),
            ("component", 406, 559),
        ],
        """mod panel;
mod dispatch;
mod block;
mod component;

pub(crate) use component::preview_nodes_for_panel;
""",
        {
            "dispatch": "use super::panel::*;\n",
            "block": "use super::dispatch::*;\nuse super::panel::*;\n",
            "component": "use super::block::*;\nuse super::dispatch::*;\nuse super::panel::*;\n",
        },
    )


def split_topbar_view() -> None:
    src = ROOT / "app/src/ui/topbar/view.rs"
    if not src.exists() or len(read_lines(src)) <= 501:
        print("SKIP topbar/view.rs")
        return
    lines = read_lines(src)
    test_start = next((i for i, l in enumerate(lines) if l.strip() == "#[cfg(test)]"), len(lines))
    out = ROOT / "app/src/ui/topbar/view"
    header = sl(lines, 1, 14)
    write(
        out / "scene_routing.rs",
        header + promote_pub_super(sl(lines, 15, 106)),
    )
    write(
        out / "app_tabs.rs",
        header
        + "use super::scene_routing::*;\n\n"
        + promote_pub_super(sl(lines, 141, 257)),
    )
    write(
        out / "mode_tabs.rs",
        header
        + "use super::scene_routing::*;\n\n"
        + promote_pub_super(sl(lines, 282, 400)),
    )
    write(
        out / "view.rs",
        header
        + "use super::app_tabs::*;\nuse super::mode_tabs::*;\nuse super::scene_routing::*;\n\n"
        + sl(lines, 108, 140)
        + sl(lines, 258, 281)
        + sl(lines, 401, test_start)
        + promote_pub_super(sl(lines, 468, 487)),
    )
    if test_start < len(lines):
        write(out / "tests.rs", sl(lines, test_start, len(lines)))
    write(
        out / "mod.rs",
        """mod scene_routing;
mod app_tabs;
mod mode_tabs;
mod view;

#[cfg(test)]
mod tests;

pub(crate) use scene_routing::{access_scene_for_topbar, append_scene_query};
pub(crate) use view::topbar_view;
""",
    )
    src.unlink()
    print("split app/src/ui/topbar/view.rs -> view/")


def main() -> None:
    split_knowledge_bundle()
    split_world_snapshot()
    split_preview_nodes()
    split_topbar_view()
    print("phase7 prod splits done")


if __name__ == "__main__":
    main()
