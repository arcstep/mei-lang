#!/usr/bin/env python3
"""Split oversized kernel/toolchain test modules (≤501 lines each)."""
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


def split_by_test_fn(src: Path, out_dir: Path, header_lines: int, chunk_names: list[str]) -> None:
    if not src.exists() or len(read_lines(src)) <= 501:
        print(f"SKIP {src.relative_to(ROOT)}")
        return
    lines = read_lines(src)
    header = sl(lines, 1, header_lines)
    body = sl(lines, header_lines + 1, len(lines))
    chunks: list[str] = []
    cur: list[str] = []
    for line in body.splitlines(keepends=True):
        if line.lstrip().startswith("#[test]") and cur:
            chunks.append("".join(cur))
            cur = []
        cur.append(line)
    if cur:
        chunks.append("".join(cur))
    if len(chunks) <= 1:
        mid = len(body.splitlines()) // 2
        bl = body.splitlines(keepends=True)
        chunks = ["".join(bl[:mid]), "".join(bl[mid:])]
    per = max(1, (len(chunks) + len(chunk_names) - 1) // len(chunk_names))
    out_dir.mkdir(parents=True, exist_ok=True)
    mods = []
    for i, name in enumerate(chunk_names):
        part = chunks[i * per : (i + 1) * per]
        if not part:
            continue
        write(out_dir / f"{name}.rs", header + "".join(part))
        mods.append(f"mod {name};")
    write(out_dir / "mod.rs", "\n".join(mods) + "\n")
    src.unlink()
    print(f"split {src.relative_to(ROOT)} -> {out_dir.relative_to(ROOT)}/")


def split_file_module(
    src_rel: str,
    out_name: str,
    header_end: int,
    sections: list[tuple[str, int, int]],
    mod_decl: str,
) -> None:
    src = ROOT / src_rel
    if not src.exists() or len(read_lines(src)) <= 501:
        print(f"SKIP {src_rel}")
        return
    lines = read_lines(src)
    out = ROOT / str(Path(src_rel).parent / out_name)
    header = sl(lines, 1, header_end)
    for name, start, end in sections:
        write(out / f"{name}.rs", header + sl(lines, start, end))
    write(out / "mod.rs", mod_decl)
    src.unlink()
    print(f"split {src_rel} -> {out_name}/")


def split_contract_regression() -> None:
    src = ROOT / "crates/toolchain/tests/contract_regression.rs"
    if not src.exists() or len(read_lines(src)) <= 501:
        print("SKIP contract_regression.rs")
        return
    lines = read_lines(src)
    out = ROOT / "crates/toolchain/tests/contract"
    header = sl(lines, 1, 40)
    # split at major test module boundaries by searching mod tests or fn test_
    boundaries = [i + 1 for i, l in enumerate(lines) if l.startswith("fn ") and "_test" in l][:1]
    if not boundaries:
        boundaries = [41]
    split_by_test_fn(
        src,
        out,
        40,
        [
            "support",
            "compile_cache",
            "world_query",
            "capability",
            "knowledge",
            "editor_runtime",
            "workspace",
        ],
    )
    mod_rs = ROOT / "crates/toolchain/tests/contract/mod.rs"
    if mod_rs.exists():
        mod_rs.write_text(
            mod_rs.read_text(encoding="utf-8")
            + "\n// integration tests re-exported from contract modules\n",
            encoding="utf-8",
        )
    # parent tests need mod contract
    lib = ROOT / "crates/toolchain/tests/contract_regression.rs"
    if not lib.exists():
        write(
            ROOT / "crates/toolchain/tests/contract_regression.rs",
            "#![allow(dead_code)]\nmod contract;\n",
        )


def main() -> None:
    split_file_module(
        "crates/kernel/src/compile/tests/authoring/panel.rs",
        "panel",
        30,
        [
            ("panel_ref", 31, 520),
            ("panel_base", 521, 1030),
            ("metric_card", 1031, 1540),
            ("links", 1541, 2036),
        ],
        "mod panel_ref;\nmod panel_base;\nmod metric_card;\nmod links;\n",
    )
    split_file_module(
        "crates/kernel/src/compile/panel_normalize/tests.rs",
        "tests",
        25,
        [
            ("helpers", 26, 320),
            ("head", 321, 615),
            ("metrics_layout", 616, 910),
            ("compound_audit", 911, 1205),
            ("vertical_align", 1206, 1481),
        ],
        "mod helpers;\nmod head;\nmod metrics_layout;\nmod compound_audit;\nmod vertical_align;\n",
    )
    split_file_module(
        "crates/kernel/src/compile/tests/examples.rs",
        "examples",
        20,
        [
            ("regressions", 21, 260),
            ("export_preview", 261, 500),
            ("cockpit_draw", 501, 740),
            ("cockpit_templates", 741, 979),
        ],
        "mod regressions;\nmod export_preview;\nmod cockpit_draw;\nmod cockpit_templates;\n",
    )
    split_file_module(
        "crates/kernel/src/compile/tests/authoring/refs.rs",
        "refs",
        25,
        [("refs_authoring", 26, 430), ("refs_scenarios", 431, 830)],
        "mod refs_authoring;\nmod refs_scenarios;\n",
    )
    split_file_module(
        "crates/kernel/src/compile/materialize/analysis_graph/tests.rs",
        "tests",
        15,
        [("closure", 16, 265), ("graph", 266, 515), ("contracts", 516, 740)],
        "mod closure;\nmod graph;\nmod contracts;\n",
    )
    split_file_module(
        "crates/kernel/src/compile/tests/cache.rs",
        "cache",
        20,
        [
            ("fixtures", 21, 155),
            ("l2_l3", 156, 290),
            ("dependency_graph", 291, 425),
            ("revision_plan", 426, 560),
            ("load_cache", 561, 679),
        ],
        "mod fixtures;\nmod l2_l3;\nmod dependency_graph;\nmod revision_plan;\nmod load_cache;\n",
    )
    split_file_module(
        "crates/kernel/src/compile/tests/entries.rs",
        "entries",
        15,
        [("scene_routes", 16, 285), ("route_warnings", 286, 551)],
        "mod scene_routes;\nmod route_warnings;\n",
    )
    split_contract_regression()
    print("kernel/toolchain test splits done")


if __name__ == "__main__":
    main()
