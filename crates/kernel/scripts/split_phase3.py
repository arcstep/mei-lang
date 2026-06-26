#!/usr/bin/env python3
"""Phase 3 kernel modularization: split large .rs files into subdirectories."""

from __future__ import annotations

import os
import re
import sys
from dataclasses import dataclass
from typing import Dict, List, Set, Tuple

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
KERNEL_SRC = os.path.normpath(os.path.join(SCRIPT_DIR, "..", "src"))

RUST_KEYWORDS = {
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
    "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
}

TOP_LEVEL_FN = re.compile(
    r"^((?:pub\s*\(\s*(?:crate|super)\s*\)\s+|pub\s+|)(?:async\s+)?fn\s+)([A-Za-z_][A-Za-z0-9_]*)"
)
CALL_RE = re.compile(r"(?<![A-Za-z0-9_])([A-Za-z_][A-Za-z0-9_]*)\s*\(")


@dataclass
class PartSpec:
    name: str
    start: int
    end: int


@dataclass
class SplitJob:
    rel_src: str
    parts: List[PartSpec]
    extra_super_depth: int = 1
    reexport: str = "pub(crate)"


SPLIT_JOBS: List[SplitJob] = [
    SplitJob("compile/projection_assembly/panel.rs", [
        PartSpec("link", 14, 479), PartSpec("params", 480, 880),
        PartSpec("shell", 881, 1066), PartSpec("shell_zones", 1067, 1411),
        PartSpec("preview", 1412, 1517),
    ], reexport="pub(crate)"),
    SplitJob("compile/projection_assembly/metric.rs", [
        PartSpec("expand_core", 8, 311), PartSpec("expand_slots", 312, 550),
        PartSpec("views", 551, 804), PartSpec("explain", 805, 1015),
        PartSpec("drilldown", 1016, 1056), PartSpec("slots", 1057, 1455),
    ], reexport="pub(super)"),
    SplitJob("compile/build_experience_index.rs", [
        PartSpec("index", 20, 133), PartSpec("reachability", 134, 316),
        PartSpec("rebuild", 317, 615), PartSpec("tree", 616, 994),
    ], extra_super_depth=0, reexport="pub"),
    SplitJob("compile/build_experience.rs", [
        PartSpec("preview", 14, 256), PartSpec("coordinate", 257, 343),
        PartSpec("path", 344, 445), PartSpec("lookup", 446, 655),
        PartSpec("overview", 656, 728),
    ], extra_super_depth=0, reexport="pub"),
    SplitJob("compile/app_compile/finish.rs", [
        PartSpec("assemble", 38, 323), PartSpec("projection", 324, 628),
        PartSpec("projection_tree", 629, 840), PartSpec("hydrate", 841, 958),
    ], extra_super_depth=0, reexport="pub(super)"),
    SplitJob("compile/build_template_index.rs", [
        PartSpec("core", 16, 288), PartSpec("helpers", 289, 521),
    ], extra_super_depth=0, reexport="pub"),
    SplitJob("mei_config/types.rs", [
        PartSpec("paths", 1, 66), PartSpec("workspace", 67, 529),
        PartSpec("app", 530, 892),
    ], extra_super_depth=0, reexport="pub"),
    SplitJob("theme_tokens.rs", [
        PartSpec("constants", 1, 56), PartSpec("validate", 57, 151),
        PartSpec("refs", 152, 539), PartSpec("literals", 540, 628),
    ], extra_super_depth=0, reexport="pub"),
    SplitJob("compile/build_node_context.rs", [
        PartSpec("preview", 15, 116), PartSpec("context", 117, 567),
        PartSpec("helpers", 568, 577),
    ], extra_super_depth=0, reexport="pub"),
    SplitJob("compile/reachability_tree.rs", [
        PartSpec("types", 1, 55), PartSpec("core", 56, 209),
        PartSpec("stock", 210, 413),
    ], extra_super_depth=0, reexport="pub"),
]


def read_lines(path: str) -> List[str]:
    with open(path, encoding="utf-8") as f:
        return f.readlines()


def write_file(path: str, content: str) -> None:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)


def extract_header(lines: List[str]) -> Tuple[List[str], int]:
    header: List[str] = []
    i = 0
    n = len(lines)
    while i < n:
        line = lines[i]
        stripped = line.strip()
        if stripped == "":
            header.append(line)
            i += 1
            continue
        if stripped.startswith("#!["):
            block = [line]
            i += 1
            while i < n and "]" not in block[-1]:
                block.append(lines[i])
                i += 1
            header.extend(block)
            continue
        if stripped.startswith("use "):
            block = [line]
            brace = line.count("{") - line.count("}")
            i += 1
            while brace > 0 and i < n:
                block.append(lines[i])
                brace += lines[i].count("{") - lines[i].count("}")
                i += 1
            header.extend(block)
            continue
        break
    return header, i + 1


def extract_test_module(lines: List[str]) -> List[str]:
    text = "".join(lines)
    idx = text.find("#[cfg(test)]")
    if idx < 0:
        return []
    start_line = text[:idx].count("\n")
    return lines[start_line:]


def adjust_super_uses(text: str, extra_depth: int) -> str:
    if extra_depth <= 0:
        return text
    extra = "super::" * extra_depth
    out: List[str] = []
    for line in text.splitlines(keepends=True):
        if line.lstrip().startswith("use super::"):
            prefix_len = len(line) - len(line.lstrip())
            rest = line[prefix_len:]
            out.append(f"{' ' * prefix_len}use {extra}{rest[len('use '):]}")
        else:
            out.append(line)
    return "".join(out)


def slice_lines(lines: List[str], start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


def top_level_fn_defs(body: str) -> Set[str]:
    names: Set[str] = set()
    for line in body.splitlines():
        if line.startswith(" ") or line.startswith("\t"):
            continue
        m = TOP_LEVEL_FN.match(line)
        if m:
            names.add(m.group(2))
    return names


def promote_top_level_fn(body: str, name: str) -> str:
    pattern = re.compile(
        rf"^((?:pub\s*\(\s*(?:crate|super)\s*\)\s+|pub\s+)?)(async\s+)?fn\s+{re.escape(name)}\b",
        re.M,
    )

    def repl(m: re.Match) -> str:
        if m.group(1):
            return m.group(0)
        return f"pub(super) {(m.group(2) or '')}fn {name}"

    return pattern.sub(repl, body, count=1)


def collect_calls(body: str, own: Set[str]) -> Set[str]:
    calls: Set[str] = set()
    for m in CALL_RE.finditer(body):
        name = m.group(1)
        if name in RUST_KEYWORDS or name in own:
            continue
        calls.add(name)
    return calls


def wire_cross_module(parts: Dict[str, str]) -> Dict[str, str]:
    defs: Dict[str, str] = {}
    for mod, body in parts.items():
        for name in top_level_fn_defs(body):
            defs[name] = mod
    updated = dict(parts)
    for mod, body in parts.items():
        own = top_level_fn_defs(body)
        needed = {c for c in collect_calls(body, own) if c in defs and defs[c] != mod}
        if not needed:
            continue
        for name in sorted(needed):
            updated[defs[name]] = promote_top_level_fn(updated[defs[name]], name)
        imports = ", ".join(sorted(needed))
        line = f"use super::{{{imports}}};\n\n"
        if line not in updated[mod]:
            updated[mod] = line + updated[mod]
    return updated


def needs_header(part: PartSpec, job: SplitJob) -> bool:
    if part.name == "constants":
        return False
    if job.rel_src.endswith("reachability_tree.rs") and part.name == "types":
        return False
    if job.rel_src.endswith("mei_config/types.rs") and part.name == "paths":
        return False
    return part.start > 1


def build_mod_rs(names: List[str], reexport: str, has_tests: bool) -> str:
    lines = [f"mod {n};" for n in names]
    if has_tests:
        lines += ["", "#[cfg(test)]", "mod tests;"]
    lines.append("")
    lines += [f"{reexport} use {n}::*;" for n in names]
    return "\n".join(lines) + "\n"


def build_tests_rs(test_lines: List[str]) -> str:
    text = "".join(test_lines)
    text = re.sub(
        r"#\[cfg\(test\)\]\s*\nmod tests \{\s*\n\s*use super::\*;\s*\n", "", text, count=1
    )
    trimmed = text.rstrip()
    if trimmed.endswith("}"):
        trimmed = trimmed[:-1].rstrip()
    return "use super::super::*;\n\n" + trimmed + "\n"


def apply_job(job: SplitJob) -> None:
    src = os.path.join(KERNEL_SRC, job.rel_src)
    if not os.path.isfile(src):
        print(f"SKIP {src}", file=sys.stderr)
        return
    lines = read_lines(src)
    header, _ = extract_header(lines)
    tests = extract_test_module(lines)
    dir_path = src[:-3]
    bodies: Dict[str, str] = {}
    for part in job.parts:
        chunk = slice_lines(lines, part.start, part.end)
        full = ("".join(header) + chunk) if needs_header(part, job) else chunk
        bodies[part.name] = adjust_super_uses(full, job.extra_super_depth)
    bodies = wire_cross_module(bodies)
    for name, content in bodies.items():
        write_file(os.path.join(dir_path, f"{name}.rs"), content)
    write_file(os.path.join(dir_path, "mod.rs"), build_mod_rs(list(bodies), job.reexport, bool(tests)))
    if tests:
        write_file(os.path.join(dir_path, "tests.rs"), build_tests_rs(tests))
    os.remove(src)
    print(f"Split {job.rel_src}")


def post_fixes() -> None:
    root = KERNEL_SRC

    def patch(rel: str, fn):
        path = os.path.join(root, rel)
        if os.path.isfile(path):
            fn(path)

    # reachability
    def fix_rt_core(p):
        t = open(p).read()
        if "use super::{ReachabilityTreeRoot" not in t:
            t = t.replace(
                "use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};\n\n",
                "use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};\n\n"
                "use super::{ReachabilityTreeRoot, is_stock_facet_root_group};\n\n",
            )
            write_file(p, t)

    patch("compile/reachability_tree/core.rs", fix_rt_core)

    def fix_rt_stock(p):
        t = open(p).read()
        t = re.sub(r"^use super::\{default\};\n\n", "", t, flags=re.M)
        if "use super::{ReachabilityTreeNode" not in t:
            t = t.replace(
                "use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};\n\n",
                "use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};\n\n"
                "use super::{ReachabilityTreeNode, ReachabilityTreeRoot};\n\n",
            )
            write_file(p, t)

    patch("compile/reachability_tree/stock.rs", fix_rt_stock)

    # build_experience_index
    for rel in ["compile/build_experience_index/rebuild.rs", "compile/build_experience_index/reachability.rs"]:
        def fix_bei(p, _rel=rel):
            t = open(p).read()
            t = t.replace("super::source_tree_enrich", "crate::compile::source_tree_enrich")
            t = t.replace("super::source_tree_world", "crate::compile::source_tree_world")
            t = t.replace("super::build_experience::", "crate::compile::build_experience::")
            t = t.replace("super::build_template_index", "crate::compile::build_template_index")
            write_file(p, t)

        patch(rel, fix_bei)

    # build_experience
    def fix_coord(p):
        t = open(p).read()
        if "BuildCompileCoordinate" in t and "use super::{BuildCompileCoordinate" not in t:
            t = "use super::{BuildCompileCoordinate, BuildPreviewKind};\n\n" + t
            write_file(p, t)

    patch("compile/build_experience/coordinate.rs", fix_coord)

    def fix_path(p):
        t = open(p).read()
        doc = "/// Human-readable breadcrumb segments for build overview / agent export.\n"
        if doc.strip() not in t:
            t = t.replace("pub fn build_experience_path", doc + "pub fn build_experience_path", 1)
            write_file(p, t)

    patch("compile/build_experience/path.rs", fix_path)

    def fix_ctx(p):
        t = open(p).read()
        if "BuildNodeContext" in t and "use super::{BuildNodeContext" not in t:
            t = "use super::{BuildNodeContext};\n\n" + t
            write_file(p, t)

    patch("compile/build_node_context/context.rs", fix_ctx)

    # panel preview doc / shell_zones orphan
    def fix_preview(p):
        t = open(p).read()
        doc = "/// Manage/build 预览：用 scene `examples[0].params` 展开 projection_slots，供无 caller 时装配 filter/chart/detail。\n"
        if doc.strip() not in t:
            t = t.replace("pub(crate) fn enrich_scene_projection_assembly_preview", doc + "pub(crate) fn enrich_scene_projection_assembly_preview", 1)
            write_file(p, t)

    patch("compile/projection_assembly/panel/preview.rs", fix_preview)

    def fix_shell_zones(p):
        t = open(p).read()
        t = re.sub(r"^/// Manage/build 预览：.*\n", "", t, flags=re.M)
        write_file(p, t)

    patch("compile/projection_assembly/panel/shell_zones.rs", fix_shell_zones)

    # types workspace/app headers and doc
    for rel in ["mei_config/types/workspace.rs", "mei_config/types/app.rs"]:
        def fix_types_hdr(p, _rel=rel):
            t = open(p).read()
            t = re.sub(r"^use super::\{is_empty\};\n\n", "", t, flags=re.M)
            if not t.lstrip().startswith("use std"):
                hdr = (
                    "use std::collections::BTreeMap;\nuse std::fs;\nuse std::path::Path;\n\n"
                    "use anyhow::{Context, Result};\nuse serde::{Deserialize, Serialize};\n"
                    "use serde_json::Value;\n\n"
                )
                t = hdr + t
            write_file(p, t)

        patch(rel, fix_types_hdr)

    def fix_ws(p):
        t = open(p).read()
        t = re.sub(r"^/// app 根目录.*\n", "", t, flags=re.M)
        write_file(p, t)

    patch("mei_config/types/workspace.rs", fix_ws)

    def fix_app(p):
        t = open(p).read()
        doc = "/// app 根目录 `.mei-config.json`：入口、路径、宿主能力与 ops。\n"
        if doc.strip() not in t and "pub struct MeiConfig" in t:
            t = t.replace("pub struct MeiConfig", doc + "pub struct MeiConfig", 1)
            write_file(p, t)

    patch("mei_config/types/app.rs", fix_app)

    # finish import depth
    finish_dir = os.path.join(root, "compile/app_compile/finish")
    if os.path.isdir(finish_dir):
        for fn in os.listdir(finish_dir):
            if not fn.endswith(".rs") or fn == "mod.rs":
                continue
            path = os.path.join(finish_dir, fn)
            t = open(path).read()
            t = t.replace("use super::active::", "use super::super::active::")
            t = t.replace("use super::catalog::CatalogCompileResult", "use super::super::catalog::CatalogCompileResult")
            for prefix in (
                "use super::super::catalog::DatasetCatalogFilter",
                "use super::super::decl_file_cache::",
                "use super::super::dependency_graph::",
                "use super::super::materialize_cache::",
                "use super::super::route_compile::",
                "use super::super::scene::",
                "use super::super::shards",
                "use super::super::{\n",
            ):
                t = t.replace(prefix, prefix.replace("super::super::", "super::super::super::", 1))
            while "super::super::super::super::" in t:
                t = t.replace("super::super::super::super::", "super::super::super::")
            t = t.replace("super::build_template_index", "crate::compile::build_template_index")
            if fn == "assemble.rs":
                pass
            elif "CompileCacheBefore" in t and "assemble::CompileCacheBefore" not in t:
                t = "use super::assemble::CompileCacheBefore;\n\n" + t
            write_file(path, t)

    # finish mod re-export assemble for siblings
    mod_path = os.path.join(finish_dir, "mod.rs")
    if os.path.isfile(mod_path):
        t = open(mod_path).read()
        if "assemble::CompileCacheBefore" not in t:
            t = t.replace(
                "pub(super) use assemble::*;",
                "pub(super) use assemble::*;\npub(super) use assemble::CompileCacheBefore;",
            )
            write_file(mod_path, t)


    fix_split_module_paths(root)


def fix_split_module_paths(root: str) -> None:
    """After split, `super::` no longer means `compile::`; fix sibling crate paths."""
    subs = [
        "compile/build_experience", "compile/build_experience_index",
        "compile/build_template_index", "compile/build_node_context",
    ]
    repls = [
        ("super::build_template_index::", "crate::compile::build_template_index::"),
        ("super::build_experience::", "crate::compile::build_experience::"),
        ("super::build_experience_index::", "crate::compile::build_experience_index::"),
        ("super::source_tree_enrich::", "crate::compile::source_tree_enrich::"),
        ("super::source_tree_world::", "crate::compile::source_tree_world::"),
        ("super::build_mcg_index::", "crate::compile::build_mcg_index::"),
        ("super::component_pack_preview::", "crate::compile::component_pack_preview::"),
        ("super::reachability_tree::", "crate::compile::reachability_tree::"),
    ]
    for sub in subs:
        d = os.path.join(root, sub)
        if not os.path.isdir(d):
            continue
        for fn in os.listdir(d):
            if not fn.endswith(".rs"):
                continue
            path = os.path.join(d, fn)
            text = open(path).read()
            orig = text
            for a, b in repls:
                text = text.replace(a, b)
            if text != orig:
                write_file(path, text)

    # metric exports
    mm = os.path.join(root, "compile/projection_assembly/metric/mod.rs")
    if os.path.isfile(mm):
        text = open(mm).read()
        for fn, old, new in [
            ("expand_core.rs", "pub(super) fn expand_board_assembly", "pub(crate) fn expand_board_assembly"),
            ("drilldown.rs", "pub(super) fn expand_drilldown_tabs", "pub(crate) fn expand_drilldown_tabs"),
            ("slots.rs", "pub(super) fn build_generic_rowset_filter_schema", "pub(crate) fn build_generic_rowset_filter_schema"),
            ("slots.rs", "pub(super) fn lower_projection_slot", "pub(crate) fn lower_projection_slot"),
        ]:
            path = os.path.join(root, "compile/projection_assembly/metric", fn)
            if os.path.isfile(path):
                t = open(path).read().replace(old, new)
                write_file(path, t)
        text = text.replace("pub(super) use", "pub(crate) use")
        if "drilldown::*" not in text:
            text = text.replace("pub(crate) use explain::*;", "pub(crate) use explain::*;\npub(crate) use drilldown::*;")
        write_file(mm, text)

    # theme constants visibility
    for rel in ["theme_tokens/validate.rs", "theme_tokens/refs.rs"]:
        path = os.path.join(root, rel)
        if os.path.isfile(path):
            t = open(path).read()
            if "use super::constants::*;" not in t:
                write_file(path, "use super::constants::*;\n\n" + t)

    # panel shell_zones helper
    path = os.path.join(root, "compile/projection_assembly/panel/shell_zones.rs")
    if os.path.isfile(path):
        t = open(path).read()
        t = re.sub(r"^/// Manage/build 预览：.*\n", "", t, flags=re.M)
        if "layout_decl_to_value" in t and "use super::{layout_decl_to_value" not in t:
            write_file(path, "use super::{layout_decl_to_value};\n\n" + t)

    # build_node_context preview orphan
    path = os.path.join(root, "compile/build_node_context/preview.rs")
    if os.path.isfile(path):
        t = open(path).read()
        t = re.sub(
            r"\n/// Resolved preview / routing context for a build-view node selection\.\n\[derive\(Debug, Clone, PartialEq, Eq\)\]\n",
            "\n",
            t,
        )
        write_file(path, t)
    path = os.path.join(root, "compile/build_node_context/context.rs")
    if os.path.isfile(path):
        t = open(path).read().replace("use super::{BuildNodeContext};\n\n", "")
        if "/// Resolved preview" not in t:
            t = t.replace(
                "pub struct BuildNodeContext",
                "/// Resolved preview / routing context for a build-view node selection.\n"
                "#[derive(Debug, Clone, PartialEq, Eq)]\n"
                "pub struct BuildNodeContext",
                1,
            )
        write_file(path, t)

    # build_experience_index index helpers from tree module
    path = os.path.join(root, "compile/build_experience_index/index.rs")
    if os.path.isfile(path):
        t = open(path).read()
        if "snapshot_to_root" in t and "tree::snapshot_to_root" not in t:
            if not t.startswith("use super::tree"):
                t = "use super::tree::{snapshot_to_root, MAX_BLOCK_CHILDREN_IN_TREE};\n\n" + t
            write_file(path, t)

    # finish visibility
    path = os.path.join(root, "compile/app_compile/finish/assemble.rs")
    if os.path.isfile(path):
        t = open(path).read()
        t = t.replace("pub(super) fn finish_compiled_app", "pub(in crate::compile::app_compile) fn finish_compiled_app")
        t = t.replace(
            "pub(super) struct CompileCacheBefore",
            "pub(in crate::compile::app_compile) struct CompileCacheBefore",
        )
        write_file(path, t)

    # mei_config app auth import
    path = os.path.join(root, "mei_config/types/app.rs")
    if os.path.isfile(path):
        t = open(path).read()
        if "WorkspaceAuthConfig" in t and "use super::workspace::WorkspaceAuthConfig" not in t:
            write_file(path, "use super::workspace::WorkspaceAuthConfig;\n\n" + t)

    # reachability core/stock imports
    for rel, imp in [
        ("compile/reachability_tree/core.rs", "use super::{ReachabilityTreeRoot, is_stock_facet_root_group};\n\n"),
        ("compile/reachability_tree/stock.rs", "use super::{ReachabilityTreeNode, ReachabilityTreeRoot};\n\n"),
    ]:
        path = os.path.join(root, rel)
        if os.path.isfile(path) and imp.strip() not in open(path).read():
            t = open(path).read()
            t = t.replace(
                "use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};\n\n",
                "use crate::model::{BuildNodeId, BuildNodeKind, CompiledApp};\n\n" + imp,
            )
            write_file(path, t)


def remove_duplicate_modules() -> None:
    pairs = [
        "compile/projection_assembly/panel", "compile/projection_assembly/metric",
        "compile/build_experience_index", "compile/build_experience",
        "compile/app_compile/finish", "compile/build_template_index",
        "compile/build_node_context", "compile/reachability_tree",
        "mei_config/types", "theme_tokens",
    ]
    for rel in pairs:
        d = os.path.join(KERNEL_SRC, rel)
        f = f"{d}.rs"
        if os.path.isdir(d) and os.path.isfile(f):
            os.remove(f)


def report_counts() -> None:
    for job in SPLIT_JOBS:
        d = os.path.join(KERNEL_SRC, job.rel_src[:-3])
        if not os.path.isdir(d):
            continue
        print(f"\n{job.rel_src} ->")
        for fn in sorted(os.listdir(d)):
            if fn.endswith(".rs"):
                with open(os.path.join(d, fn)) as fh:
                    n = sum(1 for _ in fh)
                flag = " (>500)" if n > 500 else ""
                print(f"  {fn}: {n}{flag}")


def main() -> int:
    for job in SPLIT_JOBS:
        apply_job(job)
    post_fixes()
    remove_duplicate_modules()
    report_counts()
    return 0


if __name__ == "__main__":
    sys.exit(main())
