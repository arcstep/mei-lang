//! Phase 0 compiler/Graph baseline for six Stage-architecture Golden Cases.
//!
//! Update fixtures:
//!   MEI_UPDATE_STAGE_BASELINE=1 cargo test -p mei-compiler-tests stage_architecture_baseline -- --nocapture

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mei_graph::{compile_app, CompileOutcome, GraphBlock};
use serde_json::{json, Value};

const GOLDEN_APPS: &[&str] = &[
    "mini-grid",
    "metric-grid",
    "mei-tutorial",
    "mini-data",
    "zhifa",
    "mini-park",
];

fn optional_external_workspace() -> Option<PathBuf> {
    let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
    let path = PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() || !path.is_dir() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/stage_architecture")
}

fn update_fixtures() -> bool {
    matches!(
        std::env::var("MEI_UPDATE_STAGE_BASELINE").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE")
    )
}

fn count_by_kind(blocks: &[GraphBlock]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for block in blocks {
        *counts.entry(block.kind.clone()).or_insert(0) += 1;
    }
    counts
}

fn critical_block_ids(blocks: &[GraphBlock]) -> Vec<String> {
    let mut ids: Vec<String> = blocks
        .iter()
        .filter(|b| {
            b.kind == "app_skeleton"
                || b.kind == "navigation"
                || b.kind == "presentation"
                || b.kind == "slide_layout"
                || b.kind.starts_with("slide")
                || b.block_id.starts_with("app_skeleton:")
                || b.block_id.starts_with("navigation:")
                || b.block_id.starts_with("presentation:")
                || b.block_id.contains("content_panel")
                || b.kind == "content_panel"
        })
        .map(|b| b.block_id.clone())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

fn navigation_summary(blocks: &[GraphBlock]) -> Vec<Value> {
    let mut rows = Vec::new();
    for block in blocks.iter().filter(|b| b.kind == "navigation") {
        let payload = &block.payload;
        let args = payload.get("__args").unwrap_or(payload);
        rows.push(json!({
            "block_id": block.block_id,
            "key": args.get("key").and_then(|v| v.as_str()).unwrap_or(""),
            "scene": args.get("scene").and_then(|v| v.as_str()).unwrap_or(""),
            "url": args.get("url").and_then(|v| v.as_str()).unwrap_or(""),
            "assembly": args.get("assembly")
                .or_else(|| args.get("assembly_ref"))
                .cloned()
                .unwrap_or(Value::Null),
        }));
    }
    rows.sort_by(|a, b| {
        a.get("block_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .cmp(b.get("block_id").and_then(|v| v.as_str()).unwrap_or(""))
    });
    rows
}

fn app_skeleton_summary(blocks: &[GraphBlock]) -> Value {
    blocks
        .iter()
        .find(|b| b.kind == "app_skeleton")
        .map(|b| {
            let payload = &b.payload;
            let args = payload.get("__args").unwrap_or(payload);
            json!({
                "block_id": b.block_id,
                "id": args.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                "title": args.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                "default_stage": args.get("default_stage").and_then(|v| v.as_str()).unwrap_or(""),
            })
        })
        .unwrap_or(Value::Null)
}

fn deck_slide_summary(blocks: &[GraphBlock]) -> Value {
    let slides: Vec<String> = blocks
        .iter()
        .filter(|b| {
            b.kind == "slide_layout"
                || b.block_id.starts_with("slide:")
                || b.block_id.starts_with("slide_layout:")
        })
        .map(|b| b.block_id.clone())
        .collect();
    let steps: Vec<String> = blocks
        .iter()
        .filter(|b| {
            b.kind.contains("step")
                || b.block_id.contains("@step")
                || b.block_id.contains("step:")
                || b.payload.to_string().contains("\"@step\"")
        })
        .map(|b| b.block_id.clone())
        .collect();
    let presentations: Vec<String> = blocks
        .iter()
        .filter(|b| b.kind == "presentation" || b.block_id.starts_with("presentation:"))
        .map(|b| b.block_id.clone())
        .collect();
    json!({
        "presentation_block_ids": presentations,
        "slide_block_ids": slides,
        "slide_count": slides.len(),
        "step_like_block_ids": steps,
        "step_like_count": steps.len(),
    })
}

/// Phase 1 additive: StageRegistry from access navigations (T2 excluded).
/// After 0119 closure pass, Stage-level access:* may be compiler-synthesized from MDX.
fn stage_registry_summary(blocks: &[GraphBlock], default_scene: &str) -> Value {
    let mut stages = Vec::new();
    let mut excluded_t2 = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    // Prefer stage_mdx + presentation/deck-derived access navigations.
    for block in blocks.iter().filter(|b| b.kind == "stage_mdx") {
        let payload = &block.payload;
        let stage_id = payload
            .get("stage_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if stage_id.is_empty() || !seen.insert(stage_id.to_string()) {
            continue;
        }
        let profile = payload
            .get("profile")
            .and_then(|v| v.as_str())
            .unwrap_or("cockpit");
        stages.push(json!({
            "stage_id": stage_id,
            "profile": profile,
            "is_default": stage_id == default_scene,
            "legacy_scene_id": stage_id,
        }));
    }

    for block in blocks.iter().filter(|b| b.kind == "navigation") {
        let payload = &block.payload;
        let args = payload.get("__args").unwrap_or(payload);
        let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
        if !(key.starts_with("access:") || block.block_id.starts_with("navigation:access:")) {
            continue;
        }
        let scene = args
            .get("scene")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if scene.is_empty() || !seen.insert(scene.to_string()) {
            continue;
        }
        let assembly = args
            .get("assembly")
            .or_else(|| args.get("assembly_ref"))
            .and_then(|v| {
                v.get("__args")
                    .and_then(|a| a.get("arg0"))
                    .or_else(|| v.as_str().map(|_| v))
                    .and_then(|x| x.as_str())
            })
            .unwrap_or("")
            .replace('\\', "/")
            .to_ascii_lowercase();
        if assembly.contains("/t2/")
            || assembly.contains("/overlay/")
            || assembly.contains("/plane-")
        {
            excluded_t2.push(scene.to_string());
            continue;
        }
        let profile = if assembly.contains("/presentation/")
            || assembly.contains(".deck.mdx")
            || assembly.contains(".presentation.mdx")
        {
            "slides"
        } else {
            "cockpit"
        };
        stages.push(json!({
            "stage_id": scene,
            "profile": profile,
            "is_default": scene == default_scene,
            "legacy_scene_id": scene,
        }));
    }

    excluded_t2.sort();
    let default_stage_id = stages
        .iter()
        .find(|s| {
            s.get("is_default")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .and_then(|s| s.get("stage_id").and_then(|v| v.as_str()))
        .map(str::to_string)
        .or_else(|| {
            stages
                .first()
                .and_then(|s| s.get("stage_id").and_then(|v| v.as_str()))
                .map(str::to_string)
        });

    json!({
        "stages": stages,
        "default_stage_id": default_stage_id,
        "excluded_t2_scene_ids": excluded_t2,
    })
}

/// Phase 3 additive: Graph-level ABI hints (full digests live in runtime baseline).
fn abi_summary(blocks: &[GraphBlock]) -> Value {
    let mut content_panel_ids: Vec<String> = blocks
        .iter()
        .filter(|b| b.kind == "content_panel" || b.block_id.starts_with("content_panel:"))
        .map(|b| {
            b.block_id
                .strip_prefix("content_panel:")
                .unwrap_or(&b.block_id)
                .to_string()
        })
        .collect();
    content_panel_ids.sort();
    content_panel_ids.dedup();
    let mut stage_mdx_ids: Vec<String> = blocks
        .iter()
        .filter(|b| b.kind == "stage_mdx")
        .map(|b| {
            b.payload
                .get("stage_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();
    stage_mdx_ids.sort();
    stage_mdx_ids.dedup();
    let mut stage_mdx_fills: Vec<Value> = blocks
        .iter()
        .filter(|b| b.kind == "stage_mdx")
        .flat_map(|b| {
            b.payload
                .get("fills")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
        })
        .collect();
    stage_mdx_fills.sort_by(|a, b| {
        let as_ = a.get("slot").and_then(|v| v.as_str()).unwrap_or("");
        let bs = b.get("slot").and_then(|v| v.as_str()).unwrap_or("");
        as_.cmp(bs)
    });
    json!({
        "content_panel_ids": content_panel_ids,
        "content_panel_count": content_panel_ids.len(),
        "stage_mdx_stage_ids": stage_mdx_ids,
        "stage_mdx_count": stage_mdx_ids.len(),
        "stage_mdx_fills": stage_mdx_fills,
        "digest_note": "structure_digest/narration_digest are asserted in mei-host-graph runtime baseline",
    })
}

/// Phase 2 additive: StageProgram summary derived from Registry + deck slide ids.
fn stage_programs_summary(registry: &Value, deck: &Value) -> Value {
    let slide_ids_from_deck: Vec<String> = deck
        .get("slide_block_ids")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|id| {
                    // slide_layout:app/stage/deck/p/slide-01-cover → slide-01-cover
                    id.rsplit('/').next().unwrap_or(id).to_string()
                })
                .collect()
        })
        .unwrap_or_default();

    let stages = registry
        .get("stages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut programs = Vec::new();
    for stage in stages {
        let stage_id = stage
            .get("stage_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if stage_id.is_empty() {
            continue;
        }
        let profile = stage
            .get("profile")
            .and_then(|v| v.as_str())
            .unwrap_or("cockpit");
        let source_anchor = stage
            .get("legacy_scene_id")
            .and_then(|_| stage.get("stage_id")) // placeholder; prefer explicit below
            .and_then(|v| v.as_str());
        let _ = source_anchor;
        let source = stage
            .get("source_anchor")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // compiler registry summary may not include source_anchor — recover from navigations later if empty
        let (unit_ids, unit_count) = if profile == "slides" {
            if slide_ids_from_deck.is_empty() {
                (vec![stage_id.clone()], 1)
            } else {
                (slide_ids_from_deck.clone(), slide_ids_from_deck.len())
            }
        } else {
            (vec![stage_id.clone()], 1)
        };
        programs.push(json!({
            "stage_id": stage_id,
            "profile": profile,
            "source_anchor": source,
            "unit_count": unit_count,
            "unit_ids": unit_ids,
            "state_namespace": format!("stage:{}", stage.get("stage_id").and_then(|v| v.as_str()).unwrap_or("")),
            "legacy_scene_id": stage.get("legacy_scene_id").and_then(|v| v.as_str()).unwrap_or(""),
        }));
    }
    json!({ "programs": programs })
}

fn source_files(outcome: &CompileOutcome) -> Vec<String> {
    let mut files: Vec<String> = outcome
        .files
        .iter()
        .map(|f| f.source_file.replace('\\', "/"))
        .collect();
    files.sort();
    files
}

fn normalize_summary(outcome: &CompileOutcome) -> Value {
    let kinds = count_by_kind(&outcome.blocks);
    let skeleton = app_skeleton_summary(&outcome.blocks);
    let default_stage = skeleton
        .get("default_stage")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let registry = stage_registry_summary(&outcome.blocks, default_stage);
    let deck = deck_slide_summary(&outcome.blocks);
    // Enrich program source_anchor from registry stages when present in navigations.
    let mut programs = stage_programs_summary(&registry, &deck);
    if let Some(arr) = programs.get_mut("programs").and_then(|v| v.as_array_mut()) {
        for program in arr.iter_mut() {
            let stage_id = program
                .get("stage_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if let Some(stage) =
                registry
                    .get("stages")
                    .and_then(|v| v.as_array())
                    .and_then(|stages| {
                        stages.iter().find(|s| {
                            s.get("stage_id").and_then(|v| v.as_str()) == Some(stage_id.as_str())
                        })
                    })
            {
                if let Some(anchor) = stage.get("source_anchor").and_then(|v| v.as_str()) {
                    if !anchor.is_empty() {
                        program["source_anchor"] = json!(anchor);
                    }
                }
            }
            // Fill source from access navigation assembly when registry summary lacks it.
            if program
                .get("source_anchor")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
            {
                for block in outcome.blocks.iter().filter(|b| b.kind == "navigation") {
                    let args = block.payload.get("__args").unwrap_or(&block.payload);
                    let key = args.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let scene = args.get("scene").and_then(|v| v.as_str()).unwrap_or("");
                    if scene != stage_id {
                        continue;
                    }
                    if !(key.starts_with("access:")
                        || block.block_id.starts_with("navigation:access:"))
                    {
                        continue;
                    }
                    if let Some(assembly) = args
                        .get("assembly")
                        .or_else(|| args.get("assembly_ref"))
                        .and_then(|v| {
                            v.get("__args")
                                .and_then(|a| a.get("arg0"))
                                .and_then(|x| x.as_str())
                        })
                    {
                        // home@src/scene/home.mei → src/scene/home.mei
                        let anchor = assembly
                            .split_once('@')
                            .map(|(_, path)| path)
                            .unwrap_or(assembly)
                            .replace('\\', "/");
                        program["source_anchor"] = json!(anchor);
                        break;
                    }
                }
            }
        }
    }
    json!({
        "schema_version": "mei-stage-architecture-compiler-baseline-v2",
        "app_id": outcome.app_id,
        "syntax_version": outcome.syntax_version,
        "file_count": outcome.files.len(),
        "source_files": source_files(outcome),
        "block_count": outcome.blocks.len(),
        "blocks_by_kind": kinds,
        "critical_block_ids": critical_block_ids(&outcome.blocks),
        "app_skeleton": skeleton,
        "navigations": navigation_summary(&outcome.blocks),
        "deck_or_slides": deck,
        "stage_registry": registry,
        "stage_programs": programs,
        "abi": abi_summary(&outcome.blocks),
    })
}

fn fixture_path(app_id: &str) -> PathBuf {
    fixtures_dir().join(format!("{app_id}.compiler.json"))
}

fn write_fixture(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixtures dir");
    }
    let text = serde_json::to_string_pretty(value).expect("serialize fixture");
    fs::write(path, format!("{text}\n")).expect("write fixture");
}

fn read_fixture(path: &Path) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("parse fixture")
}

#[test]
fn stage_architecture_baseline_compiles_six_goldens() {
    let Some(workspace) = optional_external_workspace() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    if !workspace.join("apps/mini-grid/app.toml").is_file() {
        eprintln!("skip: mini-grid missing under MEI_TEST_WORKSPACE");
        return;
    }

    let update = update_fixtures();
    for app_id in GOLDEN_APPS {
        let first = compile_app(&workspace, app_id)
            .unwrap_or_else(|e| panic!("compile {app_id} (pass 1): {e}"));
        let second = compile_app(&workspace, app_id)
            .unwrap_or_else(|e| panic!("compile {app_id} (pass 2): {e}"));
        let summary_a = normalize_summary(&first);
        let summary_b = normalize_summary(&second);
        assert_eq!(
            summary_a, summary_b,
            "{app_id}: dual-compile normalized summary must be identical"
        );

        let path = fixture_path(app_id);
        if update || !path.is_file() {
            write_fixture(&path, &summary_a);
            eprintln!("wrote {}", path.display());
            continue;
        }
        let expected = read_fixture(&path);
        assert_eq!(
            summary_a,
            expected,
            "{app_id}: compiler baseline mismatch.\n\
             Re-run with MEI_UPDATE_STAGE_BASELINE=1 after intentional source changes.\n\
             actual={}\nexpected={}",
            serde_json::to_string_pretty(&summary_a).unwrap_or_default(),
            serde_json::to_string_pretty(&expected).unwrap_or_default()
        );
    }
}
