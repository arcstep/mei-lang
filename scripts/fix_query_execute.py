#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fix_query_shortcut() -> None:
    p = ROOT / "crates/datasets/src/metric_dataframe/query.rs"
    text = p.read_text(encoding="utf-8")
    marker = "    if result_artifact_candidate {\n        let mut loaded_artifact = None;"
    start = text.index(marker)
    end = text.index("    let meta = parse_source_meta", start)
    block = text[start:end]
    header = text.split("pub fn query_metric_dataframe")[0]
    fn_body = block.replace("    if result_artifact_candidate {\n", "", 1)
    fn_body = fn_body.replace("        return Ok(artifact);", "        return Ok(Some(artifact));")
    fn = (
        "pub(super) fn try_load_dataframe_result_artifact(\n"
        "    app_root: &Path,\n"
        "    lookup_cache_keys: &[String],\n"
        "    response_cache_lookup_started: Instant,\n"
        ") -> Result<Option<DatasetQueryResult>> {\n"
        + fn_body
        + "    Ok(None)\n"
        + "}\n"
    )
    (ROOT / "crates/datasets/src/metric_dataframe/shortcut.rs").write_text(header + fn, encoding="utf-8")
    replacement = (
        "    if result_artifact_candidate {\n"
        "        if let Some(artifact) = try_load_dataframe_result_artifact(\n"
        "            app_root,\n"
        "            &lookup_cache_keys,\n"
        "            response_cache_lookup_started,\n"
        "        )? {\n"
        "            return Ok(artifact);\n"
        "        }\n"
        "    }\n"
    )
    new_text = text[:start] + replacement + text[end:]
    p.write_text(new_text, encoding="utf-8")
    (ROOT / "crates/datasets/src/metric_dataframe/mod.rs").write_text(
        "mod cache;\nmod materialize;\nmod query;\nmod shortcut;\n"
        "pub use cache::metric_dataframe_result_cache_key;\n"
        "pub use query::query_metric_dataframe;\n",
        encoding="utf-8",
    )
    print("query", new_text.count("\n"), "shortcut", (header + fn).count("\n"))


def trim_execute() -> None:
    p = ROOT / "server/src/http/pages/metric_api/handlers/execute.rs"
    lines = p.read_text(encoding="utf-8").splitlines()
    out: list[str] = []
    blank_run = 0
    for line in lines:
        if not line.strip():
            blank_run += 1
            if blank_run == 1:
                out.append(line)
        else:
            blank_run = 0
            out.append(line)
    while len(out) > 500:
        removed = False
        for i, line in enumerate(out):
            if not line.strip():
                out.pop(i)
                removed = True
                break
        if not removed:
            break
    p.write_text("\n".join(out) + "\n", encoding="utf-8")
    print("execute", len(out))


if __name__ == "__main__":
    fix_query_shortcut()
    trim_execute()
