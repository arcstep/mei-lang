use mei_syntax::deck::{DeckFile, DeckMarkdown, DeckSlide};
use mei_syntax::v2::{CallArgs, V2Expr, V2Item, V2SourceFile};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("line {line}: {message}")]
pub struct DeckBuildError {
    pub line: usize,
    pub message: String,
}

pub fn deck_to_v2(
    app_id: &str,
    deck_rel_path: &str,
    deck: &DeckFile,
) -> Result<V2SourceFile, DeckBuildError> {
    for slide in &deck.slides {
        if let Some(source) = &slide.source {
            return Err(DeckBuildError {
                line: source.line,
                message: format!(
                    "`@source({})` is reserved but custom slide sources are not supported yet",
                    source.path
                ),
            });
        }
    }

    let deck_root = format!("{app_id}/{}/deck", deck.frontmatter.id);
    let plane_key = format!("{deck_root}/p");
    let mut items = vec![V2Item::UseTemplate {
        path: "presentation/slide-patterns".to_string(),
        alias: Some("slide".to_string()),
    }];

    let mut presentation_keywords = vec![
        string_kw("id", &deck.frontmatter.id),
        string_kw(
            "key",
            &format!("{}@{}", deck.frontmatter.id, deck_assembly_path(deck_rel_path)),
        ),
        string_kw("title", &deck.frontmatter.title),
        string_kw(
            "summary",
            deck.frontmatter
                .summary
                .as_deref()
                .unwrap_or(&deck.frontmatter.title),
        ),
        (
            "planes".to_string(),
            V2Expr::List(vec![ref_call("plane_ref", &plane_key)]),
        ),
        ("default_script".to_string(), build_default_script(deck)),
    ];
    if let Some(theme) = &deck.frontmatter.theme {
        presentation_keywords.push((
            "theme".to_string(),
            V2Expr::RefCall {
                name: "theme_ref".to_string(),
                args: positional_string(theme),
            },
        ));
    }
    if let Some(canvas) = &deck.frontmatter.canvas {
        presentation_keywords.push(string_kw("canvas", canvas));
    }
    items.push(top_level("presentation", presentation_keywords));

    let slide_refs = deck
        .slides
        .iter()
        .map(|slide| ref_call("slide_ref", &slide_key(&deck_root, slide)))
        .collect();
    items.push(top_level(
        "plane_layout",
        vec![
            string_kw("id", "p"),
            string_kw("key", &plane_key),
            string_kw("tier", "p"),
            ("slides".to_string(), V2Expr::List(slide_refs)),
        ],
    ));

    for slide in &deck.slides {
        append_slide_items(&mut items, &deck_root, slide);
    }
    Ok(V2SourceFile { items })
}

fn append_slide_items(items: &mut Vec<V2Item>, deck_root: &str, slide: &DeckSlide) {
    let slide_key = slide_key(deck_root, slide);
    let region_key = format!("{slide_key}/r-main");
    let section_key = format!("{region_key}/s-content");
    let panel_key = format!("{section_key}/content");

    let mut slide_keywords = vec![
        string_kw("id", &slide.id),
        string_kw("key", &slide_key),
        string_kw("title", &slide.title),
        string_kw("pattern", &slide.pattern),
        (
            "regions".to_string(),
            V2Expr::List(vec![ref_call("region_ref", &region_key)]),
        ),
    ];
    if let Some(chapter) = &slide.chapter {
        slide_keywords.push(string_kw("chapter", chapter));
    }
    items.push(top_level("slide_layout", slide_keywords));

    items.push(top_level(
        "region_layout",
        vec![
            string_kw("id", "r-main"),
            string_kw("key", &region_key),
            string_kw("area", "page"),
            (
                "sections".to_string(),
                V2Expr::List(vec![ref_call("section_ref", &section_key)]),
            ),
        ],
    ));
    items.push(top_level(
        "section_layout",
        vec![
            string_kw("id", "s-content"),
            string_kw("key", &section_key),
            string_kw("area", "body"),
            (
                "blocks".to_string(),
                V2Expr::List(vec![ref_call("panel_ref", &panel_key)]),
            ),
        ],
    ));

    let blocks = slide
        .slots
        .iter()
        .map(|slot| V2Expr::Call {
            path: vec!["component".to_string()],
            args: CallArgs {
                positional: vec![V2Expr::String("mei.text".to_string())],
                keywords: vec![
                    string_kw("id", &format!("{}-{}", slide.id, slot.name)),
                    string_kw("area", &slot.name),
                    (
                        "props".to_string(),
                        V2Expr::Dict(vec![
                            (
                                "content".to_string(),
                                V2Expr::String(slot.content.html.clone()),
                            ),
                            ("format".to_string(), V2Expr::String("html".to_string())),
                            (
                                "__mei_viewpoint".to_string(),
                                V2Expr::String(slot.viewpoint_id.clone()),
                            ),
                        ]),
                    ),
                ],
            },
        })
        .collect();
    items.push(top_level(
        "content_panel",
        vec![
            string_kw("id", &format!("{}-content", slide.id)),
            string_kw("key", &panel_key),
            string_kw("scope", &panel_key),
            string_kw("variant", "container"),
            string_kw("chrome", "bare"),
            (
                "layout".to_string(),
                V2Expr::Call {
                    path: vec!["slide".to_string(), format!("{}_layout", slide.pattern)],
                    args: CallArgs::empty(),
                },
            ),
            ("blocks".to_string(), V2Expr::List(blocks)),
        ],
    ));
}

fn build_default_script(deck: &DeckFile) -> V2Expr {
    let mut steps = Vec::new();
    for slide in &deck.slides {
        steps.push(script_step(
            &format!("{}-show", slide.id),
            &slide.title,
            slide.caption.as_ref(),
            slide.speaker_notes.as_ref(),
            vec![V2Expr::Dict(vec![
                ("type".to_string(), V2Expr::String("show_page".to_string())),
                ("pageId".to_string(), V2Expr::String(slide.id.clone())),
            ])],
        ));
        for (index, step) in slide.steps.iter().enumerate() {
            steps.push(script_step(
                &format!("{}-highlight-{}", slide.id, index + 1),
                &slide.title,
                Some(&step.content),
                None,
                vec![V2Expr::Dict(vec![
                    ("type".to_string(), V2Expr::String("highlight".to_string())),
                    (
                        "viewpoint".to_string(),
                        V2Expr::String(step.viewpoint_id.clone()),
                    ),
                ])],
            ));
        }
    }
    V2Expr::Dict(vec![
        (
            "id".to_string(),
            V2Expr::String(deck.frontmatter.id.clone()),
        ),
        (
            "title".to_string(),
            V2Expr::String(deck.frontmatter.title.clone()),
        ),
        (
            "default_for_stage".to_string(),
            V2Expr::Bool(deck.frontmatter.default_for_stage),
        ),
        ("steps".to_string(), V2Expr::List(steps)),
    ])
}

fn script_step(
    id: &str,
    title: &str,
    caption: Option<&DeckMarkdown>,
    speaker_notes: Option<&DeckMarkdown>,
    actions: Vec<V2Expr>,
) -> V2Expr {
    let mut entries = vec![
        ("id".to_string(), V2Expr::String(id.to_string())),
        ("title".to_string(), V2Expr::String(title.to_string())),
        ("actions".to_string(), V2Expr::List(actions)),
    ];
    if let Some(caption) = caption {
        entries.extend([
            (
                "caption".to_string(),
                V2Expr::String(caption.markdown.clone()),
            ),
            (
                "captionMarkdown".to_string(),
                V2Expr::String(caption.markdown.clone()),
            ),
            (
                "captionHtml".to_string(),
                V2Expr::String(caption.html.clone()),
            ),
        ]);
    }
    if let Some(notes) = speaker_notes {
        entries.extend([
            (
                "speaker_notes".to_string(),
                V2Expr::String(notes.markdown.clone()),
            ),
            (
                "speakerNotesMarkdown".to_string(),
                V2Expr::String(notes.markdown.clone()),
            ),
            (
                "speakerNotesHtml".to_string(),
                V2Expr::String(notes.html.clone()),
            ),
        ]);
    }
    V2Expr::Dict(entries)
}

fn slide_key(deck_root: &str, slide: &DeckSlide) -> String {
    format!("{deck_root}/p/{}", slide.id)
}

/// Navigation `assembly_ref` uses `src/`-prefixed paths; keep presentation keys aligned.
fn deck_assembly_path(deck_rel_path: &str) -> String {
    let normalized = deck_rel_path.replace('\\', "/");
    if normalized.starts_with("src/") {
        normalized
    } else {
        format!("src/{normalized}")
    }
}

fn top_level(name: &str, keywords: Vec<(String, V2Expr)>) -> V2Item {
    V2Item::TopLevel {
        name: name.to_string(),
        args: CallArgs {
            positional: Vec::new(),
            keywords,
        },
    }
}

fn string_kw(name: &str, value: &str) -> (String, V2Expr) {
    (name.to_string(), V2Expr::String(value.to_string()))
}

fn positional_string(value: &str) -> CallArgs {
    CallArgs {
        positional: vec![V2Expr::String(value.to_string())],
        keywords: Vec::new(),
    }
}

fn ref_call(name: &str, key: &str) -> V2Expr {
    V2Expr::RefCall {
        name: name.to_string(),
        args: positional_string(key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_syntax::parse_deck_source;

    #[test]
    fn builds_ordered_v2_items_and_default_script() {
        let deck = parse_deck_source(
            r#"---
id: intro
title: Intro
default_for_stage: true
---
# First {#s1}
@template(full_bleed)
@caption
Hello.
@end
## hero {#vp_hero}
**Hero**
@step(vp_hero)
Look here.
@end
"#,
        )
        .expect("deck");
        let v2 = deck_to_v2("demo", "presentation/intro/intro.deck.mdx", &deck).expect("v2");
        assert!(matches!(
            v2.items.first(),
            Some(V2Item::UseTemplate { path, .. }) if path == "presentation/slide-patterns"
        ));
        let presentation = v2
            .items
            .iter()
            .find_map(|item| match item {
                V2Item::TopLevel { name, args } if name == "presentation" => Some(args),
                _ => None,
            })
            .expect("presentation");
        assert!(presentation.keywords.iter().any(|(key, value)| key == "key"
            && value == &V2Expr::String("intro@src/presentation/intro/intro.deck.mdx".to_string())));
        let default_script = presentation
            .keywords
            .iter()
            .find_map(|(key, value)| (key == "default_script").then_some(value))
            .expect("default_script");
        let V2Expr::Dict(script) = default_script else {
            panic!("script must be dict");
        };
        let steps = script
            .iter()
            .find_map(|(key, value)| (key == "steps").then_some(value))
            .expect("steps");
        assert!(matches!(steps, V2Expr::List(steps) if steps.len() == 2));
    }

    #[test]
    fn returns_reserved_source_diagnostic() {
        let deck = parse_deck_source(
            r#"---
id: intro
title: Intro
---
# First {#s1}
@template(full_bleed)
@source(custom/slide.mei)
## hero {#vp_hero}
Hero
"#,
        )
        .expect("source parses");
        let error = deck_to_v2("demo", "presentation/intro/intro.deck.mdx", &deck)
            .expect_err("custom source unsupported");
        assert_eq!(error.line, 7);
        assert!(error.to_string().contains("reserved"));
    }
}
