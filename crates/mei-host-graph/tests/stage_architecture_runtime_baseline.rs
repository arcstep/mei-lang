//! Phase 0 runtime assemble baseline for six Stage-architecture Golden Cases.
//!
//! Always `compile_app` → temp bundle → import/assemble（不读 env/current，避免 Gate0 浏览器污染）。
//!
//! Update fixtures:
//!   MEI_UPDATE_STAGE_BASELINE=1 cargo test -p mei-host-graph stage_architecture_runtime_baseline -- --nocapture

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mei_bundle::{compute_workspace_digest, write_bundle_from_outcome};
use mei_graph::compile_app;
use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, build_structure_full_document, clear_assemble_cache_for_app,
    collect_all_t2_page_scenes, import_bundle, list_scope_routes, ImportOptions, McgRegistryWriter,
    MrgRegistryWriter, MCG_REGISTRY_SCHEMA_VERSION, MRG_REGISTRY_SCHEMA_V3,
    SCENE_VIEW_MANIFEST_SCHEMA, STRUCTURE_FULL_SCHEMA,
};
use serde_json::{json, Value};

static IMPORT_LOCK: Mutex<()> = Mutex::new(());

struct GoldenUnit {
    app_id: &'static str,
    scene_id: &'static str,
}

const UNITS: &[GoldenUnit] = &[
    GoldenUnit {
        app_id: "mini-grid",
        scene_id: "home",
    },
    GoldenUnit {
        app_id: "metric-grid",
        scene_id: "home",
    },
    GoldenUnit {
        app_id: "mei-tutorial",
        scene_id: "intro",
    },
    GoldenUnit {
        app_id: "mini-data",
        scene_id: "home",
    },
    GoldenUnit {
        app_id: "mini-data",
        scene_id: "supervision",
    },
    GoldenUnit {
        app_id: "zhifa",
        scene_id: "home",
    },
    GoldenUnit {
        app_id: "mini-park",
        scene_id: "home",
    },
    GoldenUnit {
        app_id: "mini-park",
        scene_id: "home_2d",
    },
];

fn ws_demo_v2() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
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

fn ensure_imported(workspace: &Path, app_id: &str) {
    let _guard = IMPORT_LOCK.lock().expect("import lock");
    // 始终从源码 compile → temp bundle，避免 env/current 在 Gate0 浏览器/warmup 后漂移。
    let outcome = compile_app(workspace, app_id)
        .unwrap_or_else(|e| panic!("compile {app_id} for runtime baseline: {e}"));
    let digest = compute_workspace_digest(workspace, app_id, "stock/templates");
    let temp_dir = std::env::temp_dir().join("mei-stage-architecture-baseline");
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let bundle_path = temp_dir.join(format!("{app_id}.meibundle"));
    write_bundle_from_outcome(
        &outcome,
        digest.as_str(),
        env!("CARGO_PKG_VERSION"),
        bundle_path.as_path(),
        false,
    )
    .unwrap_or_else(|e| panic!("write bundle {app_id}: {e}"));

    let ctx = HostContext::new(workspace.to_path_buf(), app_id);
    import_bundle(
        &ctx,
        &ImportOptions {
            bundle_path: Some(bundle_path),
        },
    )
    .unwrap_or_else(|e| panic!("import {app_id}: {e}"));
    clear_assemble_cache_for_app(app_id);
}

fn schema_ledger() -> Value {
    json!({
        "mcg_registry": MCG_REGISTRY_SCHEMA_VERSION,
        "mrg_registry_host_graph": MRG_REGISTRY_SCHEMA_V3,
        "mrg_registry_server_note": "mei-mrg-registry-v2 (server/src/graph/mrg/registry.rs) forks from host-graph v3",
        "scene_view_manifest": SCENE_VIEW_MANIFEST_SCHEMA,
        "structure_full": STRUCTURE_FULL_SCHEMA,
        "manifest_index": "manifest-index-v1",
        "shell_layer": "shell-v1",
        "eval_slot_group": "eval-slot-group-v1",
        "runtime_plans": "runtime-plans-v3",
        "client_bootstrap": "mei-client-bootstrap-v1",
        "layer_plan": "mei-layer-plan-v1",
        "presentation_map": "mei-presentation-map-v1",
        "build_manifest": "mei-build-manifest-v1",
        "compile_bundle": "mei-compile-bundle-v1",
        "compiler_graph": "mei-compiler-graph-v2",
    })
}

fn route_summary(workspace: &Path, app_id: &str) -> Vec<Value> {
    let mut routes = list_scope_routes(workspace, app_id).unwrap_or_default();
    routes.sort_by(|a, b| {
        a.scene_id
            .cmp(&b.scene_id)
            .then(a.url.cmp(&b.url))
            .then(a.assembly_key.cmp(&b.assembly_key))
    });
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for route in routes {
        let key = format!("{}|{}|{}", route.scene_id, route.url, route.assembly_key);
        if seen.insert(key) {
            out.push(json!({
                "scene_id": route.scene_id,
                "url": route.url,
                "assembly_key": route.assembly_key,
            }));
        }
    }
    out
}

fn presentation_summary(map: &Value) -> Value {
    let schema = map
        .get("schemaVersion")
        .or_else(|| map.get("schema_version"))
        .cloned()
        .unwrap_or(Value::Null);
    let viewpoints = map
        .get("viewpoints")
        .and_then(|v| v.as_object())
        .map(|o| {
            let mut keys: Vec<_> = o.keys().cloned().collect();
            keys.sort();
            keys
        })
        .unwrap_or_default();
    let slides = map
        .pointer("/deck/slides")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|slide| {
                    slide
                        .get("id")
                        .or_else(|| slide.get("slideId"))
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let steps = map
        .pointer("/defaultScript/steps")
        .or_else(|| map.pointer("/default_script/steps"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.len())
        .unwrap_or(0);
    json!({
        "schemaVersion": schema,
        "slide_ids": slides,
        "slide_count": slides.len(),
        "default_script_step_count": steps,
        "viewpoint_ids": viewpoints,
        "viewpoint_count": viewpoints.len(),
    })
}

fn layer_plan_summary(plan: &Value) -> Value {
    let schema = plan
        .get("schemaVersion")
        .or_else(|| plan.get("schema_version"))
        .cloned()
        .unwrap_or(Value::Null);
    let mut tier_counts = BTreeMap::new();
    if let Some(tiers) = plan.get("tiers").and_then(|v| v.as_object()) {
        for (tier, entries) in tiers {
            tier_counts.insert(
                tier.clone(),
                entries.as_array().map(|a| a.len()).unwrap_or(0),
            );
        }
    }
    json!({
        "schemaVersion": schema,
        "tier_entry_counts": tier_counts,
    })
}

fn world_summary(plan: &Value) -> Value {
    let worlds = plan
        .get("worlds")
        .and_then(|v| v.as_object())
        .map(|o| {
            let mut ids: Vec<_> = o.keys().cloned().collect();
            ids.sort();
            ids
        })
        .unwrap_or_default();
    json!({
        "world_ids": worlds,
        "world_count": worlds.len(),
    })
}

fn panel_ids(panels: &[mei_lang_kernel::UiNodeDecl]) -> Vec<String> {
    let mut ids = Vec::new();
    fn walk(panel: &mei_lang_kernel::UiNodeDecl, out: &mut Vec<String>) {
        out.push(panel.id.clone());
        for node in &panel.blocks {
            if let mei_lang_kernel::UiTreeNode::Panel(child) = node {
                walk(child, out);
            }
        }
    }
    for panel in panels {
        walk(panel, &mut ids);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn normalize_unit(workspace: &Path, app_id: &str, scene_id: &str) -> Value {
    ensure_imported(workspace, app_id);
    let outcome = assemble_scope_from_registry(workspace, app_id, scene_id)
        .unwrap_or_else(|e| panic!("assemble {app_id}/{scene_id}: {e}"))
        .unwrap_or_else(|| panic!("missing assemble outcome for {app_id}/{scene_id}"));

    let mcg = McgRegistryWriter::load(workspace, app_id);
    let mrg = MrgRegistryWriter::load(workspace, app_id);
    let mut mcg_kinds = BTreeMap::new();
    for node in &mcg.nodes {
        *mcg_kinds
            .entry(node.id.kind.slug().to_string())
            .or_insert(0usize) += 1;
    }

    let structure = build_structure_full_document(&outcome.compiled, "phase0-baseline");
    let t2_pages = {
        let mut pages = collect_all_t2_page_scenes(workspace, app_id);
        pages.sort();
        pages
    };

    let scene_routes: Vec<Value> = outcome
        .compiled
        .scene_routes
        .iter()
        .map(|route| {
            json!({
                "scene_id": route.scene_id,
                "kind": route.kind,
                "target_file": route.target_file.replace('\\', "/"),
                "is_default": route.is_default,
            })
        })
        .collect();

    let stage_registry = {
        let registry = &outcome.compiled.stage_registry;
        let stages: Vec<Value> = registry
            .stages
            .iter()
            .map(|stage| {
                json!({
                    "stage_id": stage.id.as_str(),
                    "profile": stage.profile.as_str(),
                    "is_default": stage.is_default,
                    "legacy_scene_id": stage.legacy_scene_id,
                    "source_anchor": stage.source_anchor.replace('\\', "/"),
                })
            })
            .collect();
        let registry_ids: std::collections::BTreeSet<_> = registry
            .stages
            .iter()
            .map(|s| s.id.as_str().to_string())
            .collect();
        let excluded_t2: Vec<String> = t2_pages
            .iter()
            .filter(|id| !registry_ids.contains(id.as_str()))
            .cloned()
            .collect();
        // Gate 1: registry ids must match filtered legacy scene_routes.
        let legacy_stage_ids: Vec<String> = outcome
            .compiled
            .scene_routes
            .iter()
            .filter(|route| mei_lang_kernel::is_stage_registry_candidate(route))
            .map(|route| route.scene_id.clone())
            .collect();
        assert_eq!(
            registry.stage_ids(),
            legacy_stage_ids
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            "{app_id}/{scene_id}: stage_registry must match filtered scene_routes"
        );
        for t2 in &t2_pages {
            assert!(
                !registry.contains(t2),
                "{app_id}/{scene_id}: T2 page `{t2}` must not be in stage_registry"
            );
        }
        json!({
            "stages": stages,
            "default_stage_id": registry
                .default_stage_id
                .as_ref()
                .map(|id| id.as_str().to_string()),
            "excluded_t2_scene_ids": excluded_t2,
        })
    };

    let stage_programs = {
        let index = &outcome.compiled.stage_programs;
        let registry = &outcome.compiled.stage_registry;
        let mut program_ids = index.stage_ids();
        let mut registry_ids = registry.stage_ids();
        program_ids.sort();
        registry_ids.sort();
        assert_eq!(
            program_ids, registry_ids,
            "{app_id}/{scene_id}: stage_programs keys must match stage_registry"
        );
        for t2 in &t2_pages {
            assert!(
                !index.contains(t2),
                "{app_id}/{scene_id}: T2 page `{t2}` must not have a StageProgram"
            );
        }
        // Gate 2: active Slides stage units must match presentation_map slide order.
        if let Some(program) = index.get(scene_id) {
            if program.profile == mei_lang_kernel::StageProfile::Slides {
                let map_slide_ids = outcome
                    .presentation_map
                    .pointer("/deck/slides")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|slide| {
                                slide.get("id").and_then(|v| v.as_str()).map(str::to_string)
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if !map_slide_ids.is_empty() {
                    assert_eq!(
                        program.unit_ids(),
                        map_slide_ids.iter().map(String::as_str).collect::<Vec<_>>(),
                        "{app_id}/{scene_id}: Slides StageProgram units must match deck slide order"
                    );
                }
            }
        }
        let programs: Vec<Value> = index
            .summary_rows()
            .into_iter()
            .map(|row| {
                json!({
                    "stage_id": row.stage_id,
                    "profile": row.profile,
                    "surface": row.surface,
                    "source_anchor": row.source_anchor,
                    "unit_count": row.unit_count,
                    "unit_ids": row.unit_ids,
                    "state_namespace": row.state_namespace,
                    "legacy_scene_id": row.legacy_scene_id,
                })
            })
            .collect();
        json!({ "programs": programs })
    };

    let abi = {
        let compiled = &outcome.compiled;
        let module_key = format!("scene:{scene_id}");
        let slot_ids: Vec<String> = compiled
            .scene_slot_modules
            .get(&module_key)
            .map(|m| {
                let mut ids: Vec<_> = m.slots.iter().map(|s| s.slot_id.clone()).collect();
                ids.sort();
                ids
            })
            .unwrap_or_default();
        let mut capability_ids: Vec<String> =
            compiled.content_capabilities.keys().cloned().collect();
        capability_ids.sort();
        // Gate 3: T2 ids must not appear as public slots.
        for t2 in &t2_pages {
            assert!(
                !slot_ids.iter().any(|s| s == t2),
                "{app_id}/{scene_id}: T2 `{t2}` must not be a public semantic slot"
            );
        }
        // Gate 3 focus: public boundary = section panel_ref export only.
        if app_id == "mini-grid" {
            assert_eq!(
                slot_ids,
                vec!["mini-metric".to_string()],
                "{app_id}/{scene_id}: expected single public slot `mini-metric`"
            );
            assert_eq!(
                capability_ids,
                vec!["mini-metric".to_string()],
                "{app_id}/{scene_id}: expected single capability `mini-metric`"
            );
        }
        if app_id == "metric-grid" {
            assert_eq!(
                slot_ids,
                vec!["enforcement-compound".to_string()],
                "{app_id}/{scene_id}: expected single public slot `enforcement-compound`"
            );
            assert!(
                capability_ids.iter().any(|c| c == "enforcement-compound"),
                "{app_id}/{scene_id}: expected capability `enforcement-compound`"
            );
        }
        // Private children of compound capabilities must not be public slots.
        for cap in compiled.content_capabilities.values() {
            for private in &cap.private_child_ids {
                assert!(
                    !slot_ids.iter().any(|s| s == private),
                    "{app_id}/{scene_id}: private child `{private}` must not be a public slot"
                );
            }
        }
        let structure_digest = compiled
            .stage_programs
            .get(scene_id)
            .and_then(|p| p.structure_digest.clone());
        let narration_digest = compiled
            .stage_programs
            .get(scene_id)
            .and_then(|p| p.narration_digest.clone());
        let narr_key = format!("narration:{scene_id}");
        let narration_cue_count = compiled
            .narration_catalogs
            .get(&narr_key)
            .map(|c| c.cue_count())
            .unwrap_or(0);
        // mei-tutorial: digests present; empty script ⇒ 0 cues is ok.
        if app_id == "mei-tutorial" {
            assert!(
                structure_digest.is_some() && narration_digest.is_some(),
                "mei-tutorial: StageProgram digests must be bound"
            );
        }
        // Gate 4: mini-grid Native Stage MDX with no track ⇒ 0 cues.
        let stage_mdx_source = compiled
            .stage_programs
            .get(scene_id)
            .map(|p| p.source_anchor.replace('\\', "/"))
            .filter(|a| a.contains(".stage.mdx"));
        if app_id == "mini-grid" && scene_id == "home" {
            assert!(
                stage_mdx_source
                    .as_deref()
                    .is_some_and(|s| s.ends_with("home.stage.mdx")),
                "mini-grid: expected Native home.stage.mdx source_anchor"
            );
            assert_eq!(
                narration_cue_count, 0,
                "mini-grid: no authored Track must not synthesize cues"
            );
        }
        if app_id == "mini-data" && scene_id == "home" {
            assert!(
                narration_cue_count > 0,
                "mini-data home: Stage MDX default Track must project cues"
            );
        }
        // Gate 5: surface mapping + T2 never in registry (already asserted above).
        // Gate 6: World Content must never appear as Stage identity.
        if app_id == "mini-park" {
            for stage in &outcome.compiled.stage_registry.stages {
                let id = stage.id.as_str();
                assert!(
                    !id.contains("world-stage")
                        && id != "park_world"
                        && id != "plaza_native"
                        && id != "map-stage",
                    "{app_id}/{scene_id}: World Content `{id}` must not be a StageRegistry entry"
                );
            }
            let world_caps: Vec<_> = compiled
                .content_capabilities
                .values()
                .filter(|c| c.is_world())
                .map(|c| c.id.as_str().to_string())
                .collect();
            // World may be projected from panel ids; count is informational in fixture.
            let _ = world_caps;
            let policy = mei_lang_kernel::ProfileLayoutPolicy::for_profile(
                compiled
                    .stage_programs
                    .get(scene_id)
                    .map(|p| p.profile)
                    .unwrap_or(mei_lang_kernel::StageProfile::Cockpit),
            );
            assert_eq!(
                policy.fill_down.as_str(),
                "strict",
                "mini-park cockpit must keep strict Fill-down policy"
            );
        }
        let profile_layout_policy = compiled
            .stage_programs
            .get(scene_id)
            .map(|p| mei_lang_kernel::ProfileLayoutPolicy::for_profile(p.profile).summary_label());
        let stage_surface = compiled
            .stage_programs
            .get(scene_id)
            .map(|p| p.surface.as_str().to_string());
        if let Some(program) = compiled.stage_programs.get(scene_id) {
            let expected = match program.profile {
                mei_lang_kernel::StageProfile::Cockpit => "viewport",
                mei_lang_kernel::StageProfile::Slides => "paged",
                mei_lang_kernel::StageProfile::Page => "document",
            };
            assert_eq!(
                program.surface.as_str(),
                expected,
                "{app_id}/{scene_id}: StageSurface must match profile"
            );
        }
        json!({
            "slot_module_id": module_key,
            "public_slot_ids": slot_ids,
            "public_slot_count": compiled
                .scene_slot_modules
                .get(&module_key)
                .map(|m| m.slots.len())
                .unwrap_or(0),
            "capability_ids": capability_ids,
            "capability_count": compiled.content_capabilities.len(),
            "world_capability_count": compiled
                .content_capabilities
                .values()
                .filter(|c| c.is_world())
                .count(),
            "narration_cue_count": narration_cue_count,
            "structure_digest": structure_digest,
            "narration_digest": narration_digest,
            "stage_mdx_source": stage_mdx_source,
            "stage_surface": stage_surface,
            "profile_layout_policy": profile_layout_policy,
            "world_is_content": true,
            "digest_policy": {
                "structure_excludes_caption_notes_timing": true,
                "narration_includes_caption_notes_timing": true,
            },
        })
    };

    json!({
        "schema_version": "mei-stage-architecture-runtime-baseline-v1",
        "app_id": app_id,
        "scene_id": scene_id,
        "active_scene": outcome.compiled.active_scene,
        "assembly_key": outcome.assembly_key,
        "scene_routes": scene_routes,
        "stage_registry": stage_registry,
        "stage_programs": stage_programs,
        "abi": abi,
        "scope_routes": route_summary(workspace, app_id),
        "mcg": {
            "schemaVersion": mcg.schema_version,
            "node_count": mcg.nodes.len(),
            "nodes_by_kind": mcg_kinds,
        },
        "mrg": {
            "schemaVersion": mrg.schema_version,
            // slot_count 受 warmup/eval 污染，仅登记 schema；值见 digest_policy
            "slot_count_compared": false,
        },
        "structure_full": {
            "schema_version": structure.schema_version,
            "scene_root_count": structure.scene_roots.len(),
            "node_count": structure.nodes.len(),
            "scene_roots": structure.scene_roots,
        },
        "panel_ids": panel_ids(
            outcome
                .compiled
                .scene_contract
                .as_ref()
                .map(|c| c.panels.as_slice())
                .unwrap_or(&[])
        ),
        "layer_plan": layer_plan_summary(&outcome.layer_plan),
        "presentation_map": presentation_summary(&outcome.presentation_map),
        "world_plan": world_summary(&outcome.world_plan),
        "t2_page_scenes": t2_pages,
        "t2_page_count": t2_pages.len(),
        "schema_ledger": schema_ledger(),
        // Digests are format-only references; values may drift with env/cache.
        "digest_policy": {
            "compare_structure_counts": true,
            "compare_absolute_compile_revision": false,
            "compare_absolute_manifest_digest": false,
            "compile_revision_present": !outcome.compile_revision.is_empty(),
        },
    })
}

fn fixture_path(app_id: &str, scene_id: &str) -> PathBuf {
    fixtures_dir().join(format!("{app_id}__{scene_id}.runtime.json"))
}

fn write_fixture(path: &Path, value: &Value) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("fixtures dir");
    }
    let text = serde_json::to_string_pretty(value).expect("serialize");
    fs::write(path, format!("{text}\n")).expect("write fixture");
}

fn read_fixture(path: &Path) -> Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("parse fixture")
}

#[test]
fn stage_architecture_runtime_baseline_assembles_golden_units() {
    let Some(workspace) = ws_demo_v2() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    if !workspace.join("apps/mini-grid/app.toml").is_file() {
        eprintln!("skip: apps/mini-grid missing under MEI_TEST_WORKSPACE");
        return;
    }

    let update = update_fixtures();
    for unit in UNITS {
        let summary = normalize_unit(&workspace, unit.app_id, unit.scene_id);
        let path = fixture_path(unit.app_id, unit.scene_id);
        if update || !path.is_file() {
            write_fixture(&path, &summary);
            eprintln!("wrote {}", path.display());
            continue;
        }
        let expected = read_fixture(&path);
        assert_eq!(
            summary,
            expected,
            "{} / {}: runtime baseline mismatch.\n\
             Re-run with MEI_UPDATE_STAGE_BASELINE=1 after intentional changes.\n\
             actual={}\nexpected={}",
            unit.app_id,
            unit.scene_id,
            serde_json::to_string_pretty(&summary).unwrap_or_default(),
            serde_json::to_string_pretty(&expected).unwrap_or_default()
        );
    }
}

#[test]
fn stage_architecture_runtime_baseline_schema_ledger_constants_match_0106() {
    let ledger = schema_ledger();
    assert_eq!(
        ledger.get("mcg_registry").and_then(|v| v.as_str()),
        Some("mei-mcg-registry-v2")
    );
    assert_eq!(
        ledger
            .get("mrg_registry_host_graph")
            .and_then(|v| v.as_str()),
        Some("mei-mrg-registry-v3")
    );
    assert_eq!(
        ledger.get("scene_view_manifest").and_then(|v| v.as_str()),
        Some("scene-view-manifest-v1")
    );
    assert_eq!(
        ledger.get("structure_full").and_then(|v| v.as_str()),
        Some("structure-full-v1")
    );
}
