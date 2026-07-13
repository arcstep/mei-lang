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
@caption
Visible **caption**.
@end
@speaker_notes
Private `notes`.
@end
## claim {#vp_claim}
The **claim**.
## evidence {#vp_evidence}
- First
- Second
@step(vp_evidence)
Focus evidence.
@end

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
fn compiles_deck_as_single_source_with_order_and_script() {
    let fixture = Fixture::new(VALID_DECK);
    let outcome = compile_app(&fixture.root, "demo").expect("compile deck");

    assert_eq!(outcome.files.len(), 1);
    assert_eq!(
        outcome.files[0].source_file,
        "presentation/intro/intro.deck.mdx"
    );
    assert_eq!(outcome.files[0].blocks.len(), 10);
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
    let script = presentation
        .get("default_script")
        .and_then(Value::as_object)
        .expect("default script");
    assert_eq!(script.get("id").and_then(Value::as_str), Some("intro"));
    let steps = script
        .get("steps")
        .and_then(Value::as_array)
        .expect("script steps");
    assert_eq!(steps.len(), 3, "two show_page steps plus one highlight");
    assert_eq!(steps[0]["actions"][0]["type"].as_str(), Some("show_page"));
    assert_eq!(steps[0]["actions"][0]["pageId"].as_str(), Some("slide-01"));
    assert_eq!(steps[1]["actions"][0]["type"].as_str(), Some("highlight"));
    assert_eq!(
        steps[1]["actions"][0]["viewpoint"].as_str(),
        Some("vp_evidence")
    );
    assert_eq!(
        steps[0]["captionHtml"].as_str(),
        Some("<p>Visible <strong>caption</strong>.</p>")
    );
    assert_eq!(
        steps[0]["speakerNotesMarkdown"].as_str(),
        Some("Private `notes`.")
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
    fs::write(templates.join("slide-patterns.mei"), "template full_bleed_layout():\n    grid()\n")
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
fn reports_reserved_custom_source_as_unsupported() {
    let fixture = Fixture::new(&VALID_DECK.replace(
        "@chapter(Opening)",
        "@chapter(Opening)\n@source(custom/customer.mei)",
    ));
    let error = compile_app(&fixture.root, "demo").expect_err("custom source unsupported");
    let message = error.to_string();
    assert!(message.contains(fixture.deck.to_string_lossy().as_ref()));
    assert!(message.contains("@source(custom/customer.mei)"));
    assert!(message.contains("not supported yet"));
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
