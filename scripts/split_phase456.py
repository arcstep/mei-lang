#!/usr/bin/env python3
"""Split Phase 4-6 oversized mei-lang modules (target ≤500 lines each)."""
from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


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
    in_struct = False
    field_indent: str | None = None
    for line in content.splitlines(keepends=True):
        stripped = line.lstrip()
        indent = line[: len(line) - len(stripped)]
        if in_struct:
            if stripped.startswith("}"):
                if field_indent is None or len(indent) < len(field_indent):
                    in_struct = False
                    field_indent = None
                out.append(line)
                continue
            if field_indent is None and stripped and stripped != "{":
                field_indent = indent
            if (
                field_indent is not None
                and indent == field_indent
                and ":" in stripped
                and not stripped.startswith("pub")
                and not stripped.startswith("#")
            ):
                out.append(f"{indent}pub(super) {stripped}")
                continue
            out.append(line)
            continue
        if line and not line[0].isspace():
            if stripped.startswith("struct "):
                in_struct = True
                field_indent = None
                out.append(f"pub(super) {stripped}")
                continue
            if not stripped.startswith("pub") and (
                stripped.startswith(("fn ", "enum ", "const ", "type ", "async fn "))
            ):
                out.append(f"pub(super) {stripped}")
                continue
        if stripped.startswith("fn ") and indent == "    " and not stripped.startswith("pub"):
            out.append(f"{indent}pub(super) {stripped}")
            continue
        out.append(line)
    return "".join(out)


def split_metric_handlers() -> None:
    src = ROOT / "server/src/http/pages/metric_api/handlers.rs"
    if not src.exists():
        return
    lines = read_lines(src)
    out = ROOT / "server/src/http/pages/metric_api/handlers"
    prelude = sl(lines, 1, 37)
    for name, start, end, extra in [
        ("types", 39, 106, ""),
        ("api", 108, 456, "use super::types::*;\n"),
        ("helpers", 458, 605, "use super::types::*;\n"),
        ("execute_cache", 607, 850, "use super::types::*;\nuse super::helpers::*;\n"),
        ("execute_eval", 851, len(lines), "use super::types::*;\nuse super::helpers::*;\nuse super::execute_cache::*;\n"),
    ]:
        write(out / f"{name}.rs", prelude + extra + "\n" + promote_pub_super(sl(lines, start, end)))
    write(out / "mod.rs", "mod api;\nmod execute_cache;\nmod execute_eval;\nmod helpers;\nmod types;\n\npub use api::dataset_metric_api;\n")
    src.unlink()


def split_upload_api() -> None:
    src = ROOT / "server/src/http/upload_api.rs"
    if not src.exists():
        return
    lines = read_lines(src)
    out = ROOT / "server/src/http/upload_api"
    header = sl(lines, 1, 28)
    for name, start, end in [("types", 29, 96), ("path", 97, 318), ("chunk", 319, 586), ("download", 587, 849), ("crud", 850, len(lines))]:
        imp = header if name == "types" else header + "use super::types::*;\nuse super::path::*;\n\n"
        write(out / f"{name}.rs", imp + promote_pub_super(sl(lines, start, end)))
    write(out / "mod.rs", """mod chunk; mod crud; mod download; mod path; mod types;
pub use chunk::{upload_chunk_complete_post, upload_chunk_init_post, upload_chunk_put, upload_chunk_status_get};
pub use crud::{upload_dir_create_post, upload_entry_rename_post, upload_file_delete, upload_file_move_post, upload_file_post};
pub use download::upload_file_download_get;
""")
    src.unlink()


def split_cli_args() -> None:
    src = ROOT / "server/src/cli/args.rs"
    if not src.exists():
        return
    lines = read_lines(src)
    out = ROOT / "server/src/cli/args"
    header = "use std::path::PathBuf;\n\nuse clap::{Args, Parser, Subcommand};\n\n"
    for name, start, end in [("host_workspace", 33, 415), ("common_ops", 417, 535), ("inspect_export", 537, 657), ("query_agent", 659, len(lines))]:
        write(out / f"{name}.rs", header + sl(lines, start, end))
    write(out / "mod.rs", header + sl(lines, 5, 31) + """
mod common_ops; mod host_workspace; mod inspect_export; mod query_agent;
pub use common_ops::*; pub use host_workspace::*; pub use inspect_export::*; pub use query_agent::*;
""")
    src.unlink()


def split_file_module(src_rel: str, out_name: str, header_end: int, sections: list[tuple[str, int, int]], exports: str) -> None:
    src = ROOT / src_rel
    if not src.exists():
        return
    lines = read_lines(src)
    out = ROOT / str(Path(src_rel).parent / out_name)
    header = sl(lines, 1, header_end)
    for name, start, end in sections:
        write(out / f"{name}.rs", header + promote_pub_super(sl(lines, start, end)))
    write(out / "mod.rs", exports)
    src.unlink()


def split_result_artifact() -> None:
    split_file_module(
        "crates/datasets/src/result_artifact.rs", "result_artifact", 38,
        [("core", 39, 258), ("index_a", 259, 490), ("index_b", 491, 732), ("store", 733, 959)],
        """mod core; mod index_a; mod index_b; mod store;
pub use core::{default_result_artifact_scope, load_metric_dataframe_result_artifact, load_metric_response_result_artifact, metric_dataframe_result_artifact_exists, metric_response_result_artifact_exists, store_metric_dataframe_result_artifact, store_metric_response_result_artifact, take_metric_response_index_stats};
pub use index_a::{invalidate_prebuild_metric_response_index, prebuild_metric_response_index_covers_key};
pub use index_b::{load_prebuild_metric_response_artifact_dataset_fallback, preload_prebuild_metric_response_index, rebuild_and_install_prebuild_metric_response_index};
""",
    )


def split_metric_dataframe() -> None:
    split_file_module(
        "crates/datasets/src/metric_dataframe.rs", "metric_dataframe", 87,
        [("cache", 88, 317), ("query_a", 318, 557), ("query_b", 558, 797), ("materialize", 798, 914)],
        """mod cache; mod materialize; mod query_a; mod query_b;
pub use cache::metric_dataframe_result_cache_key; pub use query_a::query_metric_dataframe;
""",
    )


def split_paginate() -> None:
    split_file_module(
        "crates/datasets/src/paginate.rs", "paginate", 10,
        [("core", 11, 165), ("sort", 166, 287), ("filter", 288, 504), ("helpers", 505, 660)],
        """mod core; mod filter; mod helpers; mod sort;
pub(crate) use core::{paginate_rows, paginate_rows_iter};
pub(crate) use helpers::{apply_normalize, infer_columns, output_columns, row_matches, QueryWindow};
pub(crate) use sort::normalize_search;
""",
    )


def split_cache_key() -> None:
    src = ROOT / "crates/datasets/src/metric_cache_key/cache_key.rs"
    if not src.exists() or len(read_lines(src)) <= 501:
        return
    lines = read_lines(src)
    out = ROOT / "crates/datasets/src/metric_cache_key/cache_key"
    header = sl(lines, 1, 57)
    write(out / "identity.rs", header + promote_pub_super(sl(lines, 58, 260)))
    write(out / "scope.rs", header + "use super::identity::*;\n\n" + promote_pub_super(sl(lines, 261, 457)))
    write(out / "lookup.rs", header + "use super::identity::*;\nuse super::scope::*;\n\n" + promote_pub_super(sl(lines, 458, len(lines))))
    write(out / "mod.rs", """mod identity; mod lookup; mod scope;
pub(crate) use identity::{dataset_metric_identity_key, dataset_resource_lookup_aliases, effective_compile_revision_for_slot, equivalent_dataset_resource_ids, eval_node_cache_key, metric_request_revision_fingerprint, metric_request_revision_fingerprint_for_compiled, metric_scope_cache_key, serialize_cache_value, stable_slot_hash};
pub(crate) use lookup::{equivalent_dataframe_metric_scope_tokens, lookup_compiled_dataset_view, metric_dataframe_artifact_lookup_cache_keys, metric_response_artifact_lookup_cache_keys};
pub(crate) use scope::runtime_metric_eval_scope;
""")
    src.unlink()


def split_mck_tests() -> None:
    mod_path = ROOT / "crates/datasets/src/metric_cache_key/mod.rs"
    text = mod_path.read_text(encoding="utf-8")
    if "#[cfg(test)]" not in text:
        return
    idx = text.index("#[cfg(test)]")
    mod_path.write_text(text[:idx].rstrip() + "\n\n#[cfg(test)]\nmod tests;\n", encoding="utf-8")
    lines = text[idx:].replace("mod tests {", "mod tests {\n    use super::*;\n", 1).splitlines(keepends=True)
    mid = len(lines) // 2
    for i in range(mid, len(lines)):
        if lines[i].lstrip().startswith("#[test]"):
            mid = i
            break
    out = ROOT / "crates/datasets/src/metric_cache_key/tests"
    write(out / "a.rs", sl(lines, 1, mid))
    write(out / "b.rs", sl(lines, mid + 1, len(lines)))
    write(out / "mod.rs", "mod a;\nmod b;\n")


def split_lsp() -> None:
    src = ROOT / "crates/lsp/src/main.rs"
    if not src.exists() or len(read_lines(src)) <= 501:
        return
    lines = read_lines(src)
    imports = sl(lines, 3, 36)
    write(ROOT / "crates/lsp/src/diagnostics.rs", imports + "\nuse super::backend::{Backend, ValidationTrigger};\n\n" + promote_pub_super(sl(lines, 461, 851)))
    write(ROOT / "crates/lsp/src/backend.rs", imports + "\nuse crate::diagnostics::*;\n\n" + sl(lines, 39, 55) + promote_pub_super(sl(lines, 57, 460)))
    write(ROOT / "crates/lsp/src/main.rs", """mod backend; mod diagnostics; mod source_index;
use anyhow::Result; use backend::Backend; use tower_lsp::{LspService, Server};
#[tokio::main]
async fn main() -> Result<()> {
    let (service, socket) = LspService::new(Backend::new);
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service).await;
    Ok(())
}
""")


def split_preview_tests() -> None:
    src = ROOT / "app/src/ui/preview/tests.rs"
    if not src.exists():
        return
    lines = read_lines(src)
    header = sl(lines, 1, 43)
    chunks: list[str] = []
    cur: list[str] = []
    for line in sl(lines, 44, len(lines)).splitlines(keepends=True):
        if line.lstrip().startswith("#[test]") and cur:
            chunks.append("".join(cur))
            cur = []
        cur.append(line)
    if cur:
        chunks.append("".join(cur))
    out = ROOT / "app/src/ui/preview/tests"
    per = 5
    mods = []
    for i in range(0, len(chunks), per):
        name = f"g{i // per + 1}"
        write(out / f"{name}.rs", header + "".join(chunks[i : i + per]))
        mods.append(f"mod {name};")
    write(out / "mod.rs", "\n".join(mods) + "\n")
    src.unlink()


def split_world_capsule() -> None:
    src = ROOT / "app/src/ui/preview/world_capsule_preview.rs"
    if not src.exists():
        return
    lines = read_lines(src)
    header = sl(lines, 1, 19).replace("use super::", "use crate::ui::preview::")
    test_start = next((i + 1 for i, l in enumerate(lines) if l.strip() == "#[cfg(test)]"), len(lines) + 1)
    out = ROOT / "app/src/ui/preview/world_capsule"
    write(out / "lookup.rs", header + promote_pub_super(sl(lines, 20, 303)))
    write(out / "dataset.rs", header + "use super::lookup::*;\n\n" + promote_pub_super(sl(lines, 304, 513)))
    write(out / "render.rs", header + "use super::lookup::*;\nuse super::dataset::*;\n\n" + promote_pub_super(sl(lines, 514, test_start - 1)))
    if test_start <= len(lines):
        write(out / "tests.rs", sl(lines, test_start, len(lines)))
    write(out / "mod.rs", "mod dataset; mod lookup; mod render;\n#[cfg(test)] mod tests;\npub(crate) use render::world_capsule_semantic_preview;\n")
    (ROOT / "app/src/ui/preview/mod.rs").write_text((ROOT / "app/src/ui/preview/mod.rs").read_text().replace("mod world_capsule_preview;", "mod world_capsule;"), encoding="utf-8")
    v = ROOT / "app/src/ui/preview/view.rs"
    v.write_text(v.read_text().replace("use super::world_capsule_preview;", "use super::world_capsule;"), encoding="utf-8")
    src.unlink()


def split_js() -> None:
    manifest_path = ROOT / "scripts/bundle-manifest.json"
    manifest = json.loads(manifest_path.read_text())
    replacements: dict[str, list[str]] = {}
    targets = [
        "agent-panel-messages.js", "frame-stage/viewport.js", "manage-ops-panel.js",
        "spa-navigation/drilldown/swimlane-preview.js", "upload-upload-panel.js",
        "build-inspect-highlight.js", "spa-navigation/spa/loading-progress.js", "agent-panel.js",
        "page-load-progress-shell.js", "agent-panel-chrome.js", "build-navigation.js",
        "agent-panel-context.js", "spa-navigation/drilldown/widget-mount.js",
        "spa-navigation/drilldown/tab-model-config.js",
    ]
    for rel in targets:
        src = ROOT / "app/assets" / rel
        if not src.exists():
            continue
        lines = read_lines(src)
        if len(lines) <= 501:
            continue
        n = 3 if len(lines) > 900 else 2
        chunk = len(lines) // n
        parts = []
        for i in range(n):
            start = i * chunk + 1
            end = (i + 1) * chunk if i < n - 1 else len(lines)
            part_rel = str(Path(rel).with_suffix("")).replace("\\", "/") + f"/p{i + 1}.js"
            write(ROOT / "app/assets" / part_rel, sl(lines, start, end))
            parts.append(part_rel)
        src.unlink()
        replacements[rel] = parts
    for key in manifest:
        if not isinstance(manifest[key], list):
            continue
        out = []
        for e in manifest[key]:
            out.extend(replacements.get(e, [e]))
        manifest[key] = out
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")


def split_pages_tests() -> None:
    src = ROOT / "server/src/http/pages/tests.rs"
    if not src.exists() or len(read_lines(src)) <= 501:
        return
    lines = read_lines(src)
    idx = next((i for i, l in enumerate(lines) if l.strip() == "mod tests {"), None)
    if idx is None:
        return
    out = ROOT / "server/src/http/pages/tests"
    write(out / "mod.rs", "".join(lines[:idx]) + "\n#[cfg(test)]\nmod cases;\n")
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
    mods = ["use super::*;"]
    per = 6
    for i in range(0, len(chunks), per):
        name = f"c{i // per + 1}"
        write(out / f"{name}.rs", "use super::*;\n\n" + "".join(chunks[i : i + per]))
        mods.append(f"mod {name};")
    write(out / "cases.rs", "\n".join(mods) + "\n")
    src.unlink()


def split_auth_tests() -> None:
    src = ROOT / "server/src/auth/tests.rs"
    if not src.exists() or len(read_lines(src)) <= 501:
        return
    lines = read_lines(src)
    mid = len(lines) // 2
    out = ROOT / "server/src/auth/tests"
    h = sl(lines, 1, 20)
    write(out / "a.rs", h + sl(lines, 21, mid))
    write(out / "b.rs", h + sl(lines, mid + 1, len(lines)))
    write(out / "mod.rs", "mod a;\nmod b;\n")
    src.unlink()


def write_gate() -> None:
    write(ROOT / "scripts/check-max-file-lines.sh", """#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MAX=501 FAIL=0
scan(){ local l="$1"; shift; while IFS= read -r f; do [[ -z "$f" ]] && continue; n=$(wc -l < "$f" | tr -d ' ');
  [[ "$n" -gt $MAX ]] && echo "FAIL $f: $n" >&2 && FAIL=1; done < <(find "$@" \\( -path '*/target/*' -o -path '*/node_modules/*' -o -path '*/vendor/*' -o -path '*/dist/*' \\) -prune -o \\( -name '*.rs' -o -name '*.js' \\) -type f -print 2>/dev/null); }
scan server "$ROOT/server"; scan crates "$ROOT/crates"; scan app-rs "$ROOT/app/src"
scan app-js "$ROOT/app/assets" "-not" "-path" "$ROOT/app/assets/vendor/*" "-not" "-path" "$ROOT/app/assets/dist/*"
[[ $FAIL -eq 0 ]] || exit 1; echo check-max-file-lines: OK
""")
    (ROOT / "scripts/check-max-file-lines.sh").chmod(0o755)


def main() -> None:
    split_metric_handlers(); split_upload_api(); split_cli_args()
    split_result_artifact(); split_metric_dataframe(); split_paginate()
    split_cache_key(); split_mck_tests(); split_lsp()
    split_preview_tests(); split_world_capsule(); split_js()
    split_pages_tests(); split_auth_tests(); write_gate()
    print("done")


if __name__ == "__main__":
    main()
