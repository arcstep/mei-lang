use std::fs;
use std::path::{Path, PathBuf};

use mei_graph::{compile_app, CompileAppError};
use serde_json::Value;
use tempfile::TempDir;

const VALID_DECK: &str = r#"---
id: intro
title: Intro Deck
theme: presentation
canvas: 16:9
summary: Compiler fixture
default_for_stage: true
---

# Claim and evidence {#slide-01}
@template(claim_evidence)
@chapter(Opening)
## claim {#vp_claim}
The **claim**.
## evidence {#vp_evidence}
- First
- Second

# Finish {#slide-02}
@template(full_bleed)
## hero {#vp_finish}
Thank you.
"#;

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    stage: PathBuf,
    deck: PathBuf,
}

impl Fixture {
    fn new(source: &str) -> Self {
        let temp = tempfile::tempdir().expect("temp workspace");
        let root = temp.path().to_path_buf();
        let stage = root.join("apps/demo/src/presentation/intro");
        let templates = root.join("stock/templates/presentation");
        fs::create_dir_all(&stage).expect("stage");
        fs::create_dir_all(&templates).expect("templates");
        fs::write(
            root.join("workspace.json"),
            r#"{"paths":{"templates":"stock/templates"}}"#,
        )
        .expect("workspace config");
        fs::write(
            templates.join("slide-patterns.mei"),
            r#"
template full_bleed_layout():
    grid(rows = ["1fr"], columns = ["1fr"], areas = [["hero"]])

template claim_evidence_layout():
    grid(
        rows = ["1fr"],
        columns = ["1fr", "1fr"],
        areas = [["claim", "evidence"]],
    )
"#,
        )
        .expect("slide templates");
        let deck = stage.join("intro.deck.mdx");
        fs::write(&deck, source).expect("deck");
        Self {
            _temp: temp,
            root,
            stage,
            deck,
        }
    }
}

#[test]
fn compiles_deck_as_structure_only_source() {
    let fixture = Fixture::new(VALID_DECK);
    let outcome = compile_app(&fixture.root, "demo").expect("compile deck");

    let deck_file = outcome
        .files
        .iter()
        .find(|f| f.source_file.ends_with("intro.deck.mdx"))
        .expect("deck source file");
    assert_eq!(deck_file.source_file, "presentation/intro/intro.deck.mdx");
    assert_eq!(deck_file.blocks.len(), 10);
    assert_kind_count(&outcome.blocks, "presentation", 1);
    assert_kind_count(&outcome.blocks, "plane_layout", 1);
    assert_kind_count(&outcome.blocks, "slide_layout", 2);
    assert_kind_count(&outcome.blocks, "region_layout", 2);
    assert_kind_count(&outcome.blocks, "section_layout", 2);
    assert_kind_count(&outcome.blocks, "content_panel", 2);

    let presentation = block_payload(&outcome.blocks, "presentation");
    assert_eq!(
        presentation.get("key").and_then(Value::as_str),
        Some("intro@src/presentation/intro/intro.deck.mdx")
    );
    assert!(
        presentation.get("default_script").is_none(),
        "deck lowering must not create a second narration source"
    );

    let plane = block_payload(&outcome.blocks, "plane_layout");
    let slides = plane
        .get("slides")
        .and_then(Value::as_array)
        .expect("ordered slides");
    assert_eq!(
        slides[0]["__args"]["arg0"].as_str(),
        Some("demo/intro/deck/p/slide-01")
    );
    assert_eq!(
        slides[1]["__args"]["arg0"].as_str(),
        Some("demo/intro/deck/p/slide-02")
    );

    let panel = outcome
        .blocks
        .iter()
        .find(|block| {
            block
                .block_id
                .ends_with("/slide-01/r-main/s-content/content")
        })
        .expect("first content panel");
    assert_eq!(panel.payload["layout"]["__call"].as_str(), Some("grid"));
    assert_eq!(
        panel.payload["blocks"][0]["__args"]["props"]["format"].as_str(),
        Some("html")
    );
    assert_eq!(
        panel.payload["blocks"][0]["__args"]["props"]["__mei_viewpoint"].as_str(),
        Some("vp_claim")
    );
}

#[test]
fn import_parses_app_level_narration_track_independently() {
    let fixture = Fixture::new(VALID_DECK);
    let narration = fixture.root.join("apps/demo/src/narration/intro.track.mdx");
    fs::create_dir_all(narration.parent().expect("narration parent")).expect("mkdir narration");
    fs::write(
        &narration,
        r#"---
id: intro
title: Intro Narration
scope: app
default_for: [stage:intro]
---
@cue(stage:intro/slide:slide-01)
@caption
Visible caption.
@end
@end
"#,
    )
    .expect("write narration");
    let outcome = compile_app(&fixture.root, "demo").expect("compile track");
    let file = outcome
        .files
        .iter()
        .find(|file| file.source_file == "narration/intro.track.mdx")
        .expect("independently parsed narration source");
    assert!(
        file.blocks.is_empty(),
        "Track does not lower to PageProgram graph"
    );
}

#[test]
fn rejects_invalid_pattern_and_slot_with_path_and_line() {
    let invalid_pattern =
        Fixture::new(&VALID_DECK.replace("@template(claim_evidence)", "@template(two_columns)"));
    let error = compile_app(&invalid_pattern.root, "demo").expect_err("invalid pattern");
    let message = error.to_string();
    assert!(message.contains(invalid_pattern.deck.to_string_lossy().as_ref()));
    assert!(message.contains(":11:1"));
    assert!(message.contains("unknown slide pattern `two_columns`"));

    let invalid_slot = Fixture::new(&VALID_DECK.replace("## evidence", "## action"));
    let error = compile_app(&invalid_slot.root, "demo").expect_err("invalid slot");
    let message = error.to_string();
    assert!(message.contains(invalid_slot.deck.to_string_lossy().as_ref()));
    assert!(message.contains("unknown slot `action`"));
}

#[test]
fn rejects_deck_and_presentation_tree_as_dual_sources() {
    let fixture = Fixture::new(VALID_DECK);
    let presentation = fixture.stage.join("presentation.mei");
    fs::write(&presentation, "presentation(id = \"intro\")").expect("presentation");
    let error = compile_app(&fixture.root, "demo").expect_err("dual root source");
    assert_dual_source(error, &fixture.deck, &presentation);

    fs::remove_file(&presentation).expect("remove presentation");
    let slide = fixture.stage.join("p/slide-01.mei");
    fs::create_dir_all(slide.parent().expect("slide parent")).expect("p");
    fs::write(&slide, "slide_layout(id = \"slide-01\")").expect("slide tree");
    let error = compile_app(&fixture.root, "demo").expect_err("dual slide source");
    assert_dual_source(error, &fixture.deck, &slide);
}

#[test]
fn rejects_deck_and_default_presentation_mdx() {
    let fixture = Fixture::new(VALID_DECK);
    let script = fixture.stage.join("intro.presentation.mdx");
    fs::write(&script, "---\npresentation: intro\n---\n## step\n").expect("presentation mdx");
    let error = compile_app(&fixture.root, "demo").expect_err("deck + presentation.mdx");
    assert_dual_source(error, &fixture.deck, &script);
}

#[test]
fn rejects_legacy_presentation_tree_without_deck() {
    let temp = tempfile::tempdir().expect("temp workspace");
    let root = temp.path().to_path_buf();
    let stage = root.join("apps/demo/src/presentation/intro");
    let templates = root.join("stock/templates/presentation");
    fs::create_dir_all(&stage).expect("stage");
    fs::create_dir_all(&templates).expect("templates");
    fs::write(
        root.join("workspace.json"),
        r#"{"paths":{"templates":"stock/templates"}}"#,
    )
    .expect("workspace config");
    fs::write(
        templates.join("slide-patterns.mei"),
        "template full_bleed_layout():\n    grid()\n",
    )
    .expect("templates");
    let presentation = stage.join("presentation.mei");
    fs::write(&presentation, "presentation(id = \"intro\")").expect("legacy root");
    let error = compile_app(&root, "demo").expect_err("legacy alone");
    let message = error.to_string();
    assert!(message.contains("presentation_dual_source_forbidden"));
    assert!(message.contains(presentation.to_string_lossy().as_ref()));
}

#[test]
fn rejects_second_deck_in_same_stage() {
    let fixture = Fixture::new(VALID_DECK);
    let second = fixture.stage.join("alt.deck.mdx");
    fs::write(&second, VALID_DECK).expect("second deck");
    let error = compile_app(&fixture.root, "demo").expect_err("second deck");
    let message = error.to_string();
    assert!(message.contains("presentation_dual_source_forbidden"));
    assert!(message.contains("two authoring sources"));
}

#[test]
fn compiles_custom_source_fragment_into_non_text_blocks() {
    let fixture = Fixture::new(
        r#"---
id: intro
title: Intro Deck
theme: presentation
canvas: 16:9
summary: Compiler fixture
default_for_stage: true
---

# Graph page {#slide-01}
@template(claim_evidence)
@chapter(Opening)
@source(custom/graph.mei#graph_page)
"#,
    );
    let custom_dir = fixture.stage.join("custom");
    fs::create_dir_all(&custom_dir).expect("custom");
    fs::write(
        custom_dir.join("graph.mei"),
        r#"
template graph_page():
    [
        component(
            "mei.text",
            id = "claim",
            area = "claim",
            props = {"content": "Graph claim", "format": "html"},
        ),
        component(
            "chart.column",
            id = "evidence-chart",
            area = "evidence",
            props = {
                "title": "Bundle layers",
                "chartHeight": 220,
                "data": {
                    "rows": [
                        {"layer": "layer_plan", "count": 1},
                        {"layer": "presentation_map", "count": 1},
                        {"layer": "registry", "count": 1},
                    ],
                },
                "mapping": {
                    "x": [{"field": "layer", "name": "layer"}],
                    "y": [{"field": "count", "name": "count"}],
                },
            },
        ),
    ]
"#,
    )
    .expect("custom source");
    let outcome = compile_app(&fixture.root, "demo").expect("compile with @source");
    let panels: Vec<_> = outcome
        .blocks
        .iter()
        .filter(|block| block.kind == "content_panel")
        .collect();
    assert_eq!(panels.len(), 1);
    let blocks = panels[0]
        .payload
        .get("blocks")
        .and_then(|value| value.as_array())
        .expect("panel blocks");
    assert_eq!(blocks.len(), 2);
    let use_keys: Vec<&str> = blocks
        .iter()
        .filter_map(|block| {
            block
                .pointer("/__args/arg0")
                .and_then(|value| value.as_str())
                .or_else(|| block.get("use_key").and_then(|value| value.as_str()))
        })
        .collect();
    assert!(
        use_keys.iter().any(|key| *key == "chart.column"),
        "expected chart.column use_key in blocks, got {blocks:?}"
    );
}

#[test]
fn rejects_custom_source_missing_fragment_file() {
    let fixture = Fixture::new(
        r#"---
id: intro
title: Intro Deck
theme: presentation
canvas: 16:9
summary: Compiler fixture
default_for_stage: true
---

# Graph page {#slide-01}
@template(full_bleed)
@source(custom/missing.mei#hero_blocks)
"#,
    );
    let error = compile_app(&fixture.root, "demo").expect_err("missing source");
    let message = error.to_string();
    assert!(message.contains(fixture.deck.to_string_lossy().as_ref()));
    assert!(message.contains("deck_source_missing") || message.contains("not found"));
}

fn assert_kind_count(blocks: &[mei_graph::GraphBlock], kind: &str, expected: usize) {
    assert_eq!(
        blocks.iter().filter(|block| block.kind == kind).count(),
        expected,
        "unexpected `{kind}` block count"
    );
}

fn block_payload<'a>(blocks: &'a [mei_graph::GraphBlock], kind: &str) -> &'a Value {
    &blocks
        .iter()
        .find(|block| block.kind == kind)
        .unwrap_or_else(|| panic!("missing `{kind}` block"))
        .payload
}

fn assert_dual_source(error: CompileAppError, deck: &Path, existing: &Path) {
    let message = error.to_string();
    assert!(
        message.contains("presentation_dual_source_forbidden"),
        "missing diagnostic code in: {message}"
    );
    assert!(message.contains("two authoring sources"));
    assert!(message.contains(deck.to_string_lossy().as_ref()));
    assert!(message.contains(existing.to_string_lossy().as_ref()));
}
