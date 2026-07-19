use std::path::PathBuf;
use std::sync::Once;

use mei_host_core::HostContext;
use mei_host_graph::{
    assemble_scope_from_registry, build_eval_slot_group_document, build_structure_full_document,
    clear_assemble_cache_for_app, collect_slot_groups, import_bundle, ImportOptions,
    StructureFullDocument,
};
use mei_lang_kernel::DataMode;

static INIT: Once = Once::new();

fn ws_demo_v2() -> Option<PathBuf> {
    mei_test_support::optional_external_workspace()
}

fn ensure_zhifa_imported() -> Option<PathBuf> {
    let workspace = ws_demo_v2()?;
    INIT.call_once(|| {
        let bundle = workspace.join("apps/zhifa/env/current/build/exchange/zhifa.meibundle");
        if !bundle.is_file() {
            return;
        }
        let ctx = HostContext::new(workspace.clone(), "zhifa");
        import_bundle(
            &ctx,
            &ImportOptions {
                bundle_path: Some(bundle),
            },
        )
        .expect("import zhifa bundle");
        clear_assemble_cache_for_app("zhifa");
    });
    Some(workspace)
}

fn assemble_zhifa_home() -> Option<(PathBuf, mei_host_graph::AssembleOutcome)> {
    let workspace = ensure_zhifa_imported()?;
    let outcome = assemble_scope_from_registry(workspace.as_path(), "zhifa", "home")
        .expect("assemble")
        .expect("home outcome");
    Some((workspace, outcome))
}

fn structure_scope_count(structure: &StructureFullDocument, needle: &str) -> usize {
    structure
        .nodes
        .iter()
        .filter(|node| node.preview_scope.contains(needle))
        .count()
}

#[test]
fn zhifa_enforcement_and_issue_export_body_structure() {
    let Some((_workspace, outcome)) = assemble_zhifa_home() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };

    let structure = build_structure_full_document(&outcome.compiled, "test");
    assert!(
        structure_scope_count(&structure, "enforcement_strip_layout") >= 4,
        "enforcement strip should expand triptych + compound slots, got scopes: {:?}",
        structure
            .nodes
            .iter()
            .filter(|node| node.preview_scope.contains("enforcement"))
            .map(|node| node.preview_scope.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        structure.nodes.iter().any(|node| {
            node.preview_scope.contains("enforcement_strip_layout")
                && node.preview_scope.ends_with("/top")
        }),
        "enforcement compound top metric should appear in structure, got scopes: {:?}",
        structure
            .nodes
            .iter()
            .filter(|node| node.preview_scope.contains("enforcement_strip_layout"))
            .map(|node| node.preview_scope.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        structure_scope_count(&structure, "issue_body") >= 8,
        "issue body should expand into status cards, got scopes: {:?}",
        structure
            .nodes
            .iter()
            .filter(|node| node.preview_scope.contains("issue"))
            .map(|node| node.preview_scope.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn zhifa_section_head_eval_slots_do_not_aggregate_scene_mounts() {
    let Some((workspace, outcome)) = assemble_zhifa_home() else {
        eprintln!("skip: set MEI_TEST_WORKSPACE for private demo probes");
        return;
    };
    let structure = build_structure_full_document(&outcome.compiled, "test");

    for scope_suffix in [
        "t1/left_rail/enforcement/head/mei.text",
        "t1/right_rail/issue/head/mei.text",
        "t1/right_rail/warning/head/mei.text",
    ] {
        let group = format!("content:{scope_suffix}");
        let doc = build_eval_slot_group_document(
            &outcome.compiled,
            &structure,
            group.as_str(),
            DataMode::Eval,
            Some(workspace.as_path()),
        );
        let mounts = doc
            .slots
            .get(scope_suffix)
            .and_then(|slot| slot.get("component_mounts"))
            .and_then(|value| value.as_array())
            .map(|mounts| mounts.len())
            .unwrap_or(0);
        assert!(
            mounts <= 1,
            "head slot {scope_suffix} should not aggregate scene mounts, got {mounts}"
        );
    }

    let _ = collect_slot_groups(&structure);
}
