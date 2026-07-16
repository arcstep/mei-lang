use std::fs;
use std::path::PathBuf;

use mei_graph::compile_app;

#[test]
fn compiles_objects_mei_into_object_catalog_graph_block() {
    let temp = tempfile::tempdir().expect("temp workspace");
    let root = temp.path();
    let source_dir = root.join("apps/demo/src/domain");
    fs::create_dir_all(&source_dir).expect("source directory");
    fs::create_dir_all(root.join("stock/templates")).expect("template directory");
    fs::write(
        root.join("workspace.json"),
        r#"{"paths":{"templates":"stock/templates"}}"#,
    )
    .expect("workspace config");
    fs::write(
        source_dir.join("warnings.objects.mei"),
        r#"
object_catalog(
    id = "warning_objects",
    types = [
        object_type(
            id = "zhifa.Warning",
            identity = object_identity(
                fields = ["warning_id"],
                aliases = ["warningId"],
            ),
            source = dataset_ref("warning_rows"),
            capabilities = ["select", "explain"],
        ),
    ],
)
"#,
    )
    .expect("object catalog source");

    let outcome = compile_app(root, "demo").expect("compile object catalog");
    let source = outcome
        .files
        .iter()
        .find(|file| file.source_file == "domain/warnings.objects.mei")
        .expect(".objects.mei source outcome");
    let block = source
        .blocks
        .iter()
        .find(|block| block.kind == "object_catalog")
        .expect("object_catalog GraphBlock");

    assert_eq!(block.schema, "mei-object-catalog-v1");
    assert_eq!(block.block_id, "object_catalog:warning_objects");
    assert_eq!(block.payload["types"][0]["id"], "zhifa.Warning");
    assert_eq!(
        block.payload["types"][0]["identity"]["aliases"][0],
        "warningId"
    );
    assert_eq!(block.payload["types"][0]["capabilities"][0], "select");
    assert_eq!(
        block.payload["source_anchor"],
        "domain/warnings.objects.mei"
    );
    assert_eq!(block.payload["authoring_mode"], "legacy");
    assert_eq!(
        block.payload["diagnostics"][0]["code"],
        "object_catalog_legacy_authoring"
    );
}

#[test]
fn compiles_high_level_object_intent_into_internal_catalog_index_and_assembly() {
    let temp = tempfile::tempdir().expect("temp workspace");
    let root = temp.path();
    let source_dir = root.join("apps/demo/src/domain");
    fs::create_dir_all(&source_dir).expect("source directory");
    fs::create_dir_all(root.join("stock/templates")).expect("template directory");
    fs::write(
        root.join("workspace.json"),
        r#"{"paths":{"templates":"stock/templates"}}"#,
    )
    .expect("workspace config");
    fs::write(
        source_dir.join("alerts.objects.mei"),
        include_str!("fixtures/object_intent.mei"),
    )
    .expect("object intent fixture");

    let outcome = compile_app(root, "demo").expect("compile object intent");
    let block = outcome
        .blocks
        .iter()
        .find(|block| block.kind == "object_catalog")
        .expect("object intent catalog block");

    assert_eq!(block.schema, "mei-object-catalog-v1");
    assert_eq!(block.payload["authoring_mode"], "author_intent");
    assert_eq!(block.payload["types"][0]["id"], "ops.Alert");
    assert_eq!(
        block.payload["types"][0]["identity"]["locator"]["kind"],
        "field_ref"
    );
    assert_eq!(block.payload["intents"][0]["recipe"]["id"], "alert");
    assert_eq!(block.payload["index"][0]["kind"], "internal_object_index");
    assert_eq!(
        block.payload["default_assemblies"][0]["kind"],
        "default_object_assembly"
    );
    assert_eq!(
        block.payload["diagnostics"][0]["code"],
        "object_intent_extension_slot"
    );
    assert!(block.payload["intents"][0].get("payload").is_none());
    assert!(block.payload["index"][0]["owner_hints"]
        .as_array()
        .is_some_and(|hints| hints.iter().any(|hint| hint["id"] == "alerts")));
    let bindings = block.payload["interaction_bindings"]
        .as_array()
        .expect("derived interaction bindings");
    let row = bindings
        .iter()
        .find(|binding| binding["trigger"] == "row_click")
        .expect("row click binding");
    assert_eq!(row["intents"], serde_json::json!(["select"]));
    assert_eq!(row["priority"], "derived");
    let detail = bindings
        .iter()
        .find(|binding| binding["trigger"] == "detail_click")
        .expect("detail binding");
    assert_eq!(
        detail["intents"],
        serde_json::json!(["select", "open_projection"])
    );
    assert_eq!(detail["priority"], "explicit_link");
    let metric = bindings
        .iter()
        .find(|binding| binding["trigger"] == "explain_click")
        .expect("explain click binding");
    assert_eq!(metric["subjectKind"], "object_set");
    assert_eq!(metric["target"]["id"], "open_alerts");
    let map = bindings
        .iter()
        .find(|binding| binding["trigger"] == "map_world_pick")
        .expect("map/world binding");
    assert_eq!(
        map["intents"],
        serde_json::json!(["select", "focus_viewpoint"])
    );
    assert!(!map["intents"]
        .as_array()
        .expect("map intents")
        .iter()
        .any(|intent| intent == "open_projection"));
    assert!(block.payload["responders"]
        .as_array()
        .is_some_and(|responders| responders.iter().any(|item| item["role"] == "explain")));
    let assembly = &block.payload["default_assemblies"][0];
    assert_eq!(
        assembly["recipe_contract"]["schema_version"],
        "mei-stock-object-recipe-v1"
    );
    assert_eq!(assembly["recipe_contract"]["id"], "cockpit.alert");
    assert_eq!(
        assembly["recipe_contract"]["override_precedence"],
        serde_json::json!([
            "local",
            "domain",
            "app",
            "stock",
            "placeholder",
            "no_projection"
        ])
    );
    assert_eq!(assembly["effective_override"]["density"], "dense");
    assert_eq!(assembly["override_sources"]["density"], "local");
    assert!(assembly["projections"]
        .as_array()
        .is_some_and(|projections| projections
            .iter()
            .all(|projection| projection["state"] == "ready")));
    let serialized = serde_json::to_string(assembly).expect("serialize recipe assembly");
    for forbidden in [
        "\"rows\"",
        "\"payload\"",
        "\"option\"",
        "\"geojson\"",
        "\"geometry\"",
        "\"html\"",
        "\"script\"",
    ] {
        assert!(
            !serialized.to_ascii_lowercase().contains(forbidden),
            "recipe assembly retained forbidden owner payload key {forbidden}"
        );
    }

    let first = serde_json::to_string(&block.payload).expect("serialize first payload");
    let second_outcome = compile_app(root, "demo").expect("compile object intent again");
    let second = second_outcome
        .blocks
        .iter()
        .find(|block| block.kind == "object_catalog")
        .expect("second object intent catalog block");
    assert_eq!(
        first,
        serde_json::to_string(&second.payload).expect("serialize second payload")
    );
}

#[test]
fn compiles_case_place_and_event_stock_recipe_contracts() {
    for (fixture, expected_type, recipe) in [
        ("object_intent_master_detail.mei", "ops.Case", "case"),
        ("object_intent_place.mei", "ops.Place", "place"),
        ("object_intent_ambiguous.mei", "ops.Event", "event"),
    ] {
        let temp = tempfile::tempdir().expect("temp workspace");
        let root = temp.path();
        let source_dir = root.join("apps/demo/src/domain");
        fs::create_dir_all(&source_dir).expect("source directory");
        fs::create_dir_all(root.join("stock/templates")).expect("template directory");
        fs::write(
            root.join("workspace.json"),
            r#"{"paths":{"templates":"stock/templates"}}"#,
        )
        .expect("workspace config");
        fs::write(
            source_dir.join("objects.objects.mei"),
            fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures")
                    .join(fixture),
            )
            .expect("read fixture"),
        )
        .expect("write object fixture");

        let outcome = compile_app(root, "demo").expect("compile object fixture");
        let block = outcome
            .blocks
            .iter()
            .find(|block| block.kind == "object_catalog")
            .expect("object catalog block");
        assert_eq!(block.payload["types"][0]["id"], expected_type);
        let assembly = &block.payload["default_assemblies"][0];
        assert_eq!(
            assembly["recipe_contract"]["id"],
            format!("cockpit.{recipe}")
        );
        assert_eq!(assembly["recipe_contract"]["identity_locked"], true);
        assert!(assembly["projections"]
            .as_array()
            .is_some_and(|projections| !projections.is_empty()));
        if recipe == "case" {
            assert_eq!(
                assembly["recipe_contract"]["privacy_notice"],
                "attachments and evidence are PII-redacted by default; owners must opt in to reveal"
            );
            assert_eq!(
                assembly["effective_override"]["privacyMode"],
                "pii_redacted"
            );
        }
        if recipe == "place" {
            assert_eq!(
                assembly["slots"]["entityId"]["kind"], "entity_id",
                "place entityId slot should derive from the identity locator"
            );
            assert!(assembly["projections"]
                .as_array()
                .expect("place projections")
                .iter()
                .any(|projection| {
                    projection["role"] == "world"
                        && projection["reuses"][0]["id"] == "cockpit.world-stage"
                }));
        }
        if recipe == "event" {
            assert!(block.payload["interaction_bindings"]
                .as_array()
                .expect("event bindings")
                .iter()
                .all(|binding| binding["selectionMode"] == "secondary"));
            assert!(assembly["projections"]
                .as_array()
                .expect("event projections")
                .iter()
                .any(|projection| {
                    projection["role"] == "timeline"
                        && projection["reuses"][0]["id"] == "thunder.playback-strip"
                }));
        }
    }
}

#[test]
fn missing_slots_degrade_without_fabricating_inputs() {
    let temp = tempfile::tempdir().expect("temp workspace");
    let root = temp.path();
    let source_dir = root.join("apps/demo/src/domain");
    fs::create_dir_all(&source_dir).expect("source directory");
    fs::create_dir_all(root.join("stock/templates")).expect("template directory");
    fs::write(
        root.join("workspace.json"),
        r#"{"paths":{"templates":"stock/templates"}}"#,
    )
    .expect("workspace config");
    fs::write(
        source_dir.join("alerts.objects.mei"),
        r#"
object(
    type = "ops.Alert",
    source = dataset_ref("alerts"),
    identity = field_ref("alert_id"),
    recipe = stock_ref("alert"),
    slots = {"label": field_ref("title")},
)
"#,
    )
    .expect("write degraded recipe");

    let outcome = compile_app(root, "demo").expect("compile degraded recipe");
    let block = outcome
        .blocks
        .iter()
        .find(|block| block.kind == "object_catalog")
        .expect("object catalog");
    let assembly = &block.payload["default_assemblies"][0];
    let list = assembly["projections"]
        .as_array()
        .expect("projections")
        .iter()
        .find(|projection| projection["role"] == "list")
        .expect("list projection");
    assert_eq!(list["state"], "degraded");
    assert_eq!(
        list["missing_slots"],
        serde_json::json!(["severity", "occurredAt", "status"])
    );
    assert_eq!(list["inputs"].as_object().expect("inputs").len(), 1);
    assert_eq!(
        block.payload["diagnostics"][0]["code"],
        "object_recipe_required_slots_missing"
    );
}

#[test]
fn identity_override_is_a_compile_error() {
    let temp = tempfile::tempdir().expect("temp workspace");
    let root = temp.path();
    let source_dir = root.join("apps/demo/src/domain");
    fs::create_dir_all(&source_dir).expect("source directory");
    fs::create_dir_all(root.join("stock/templates")).expect("template directory");
    fs::write(
        root.join("workspace.json"),
        r#"{"paths":{"templates":"stock/templates"}}"#,
    )
    .expect("workspace config");
    fs::write(
        source_dir.join("alerts.objects.mei"),
        r#"
object(
    type = "ops.Alert",
    source = dataset_ref("alerts"),
    identity = field_ref("alert_id"),
    recipe = stock_ref("alert"),
    override = {"local": {"identity": "other_id"}},
)
"#,
    )
    .expect("write invalid override");

    let error = compile_app(root, "demo").expect_err("identity override must fail");
    assert!(
        error
            .to_string()
            .contains("object_intent_identity_override_forbidden"),
        "unexpected error: {error}"
    );
}

#[test]
fn legacy_catalog_has_zero_recipe_migration() {
    let temp = tempfile::tempdir().expect("temp workspace");
    let root = temp.path();
    let source_dir = root.join("apps/demo/src/domain");
    fs::create_dir_all(&source_dir).expect("source directory");
    fs::create_dir_all(root.join("stock/templates")).expect("template directory");
    fs::write(
        root.join("workspace.json"),
        r#"{"paths":{"templates":"stock/templates"}}"#,
    )
    .expect("workspace config");
    fs::write(
        source_dir.join("legacy.objects.mei"),
        r#"object_catalog(id = "legacy", types = [])"#,
    )
    .expect("legacy source");

    let outcome = compile_app(root, "demo").expect("compile legacy catalog");
    let block = outcome
        .blocks
        .iter()
        .find(|block| block.kind == "object_catalog")
        .expect("legacy object catalog");
    assert_eq!(block.payload["authoring_mode"], "legacy");
    assert_eq!(block.payload["default_assemblies"], serde_json::json!([]));
    assert_eq!(block.payload["interaction_bindings"], serde_json::json!([]));
    assert_eq!(block.payload["responders"], serde_json::json!([]));
}
