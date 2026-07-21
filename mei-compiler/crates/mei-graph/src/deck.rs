use std::collections::BTreeMap;
use std::path::Path;

use mei_syntax::deck::{DeckFile, DeckSlide, DeckSource};
use mei_syntax::v2::{parse_v2_source, CallArgs, V2Expr, V2Item, V2SourceFile};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("line {line}: {message}")]
pub struct DeckBuildError {
    pub line: usize,
    pub message: String,
}

const FORBIDDEN_SOURCE_ROOTS: &[&str] = &[
    "presentation",
    "plane_layout",
    "slide_layout",
    "region_layout",
    "section_layout",
];

pub fn deck_to_v2(
    app_id: &str,
    deck_rel_path: &str,
    deck: &DeckFile,
) -> Result<V2SourceFile, DeckBuildError> {
    deck_to_v2_with_dir(app_id, deck_rel_path, deck, None)
}

/// Lower a deck AST to in-memory v2. When `deck_dir` is provided, `@source` files are
/// validated on disk (`custom/*.mei#fragment` must exist and must not declare stage roots).
pub fn deck_to_v2_with_dir(
    app_id: &str,
    deck_rel_path: &str,
    deck: &DeckFile,
    deck_dir: Option<&Path>,
) -> Result<V2SourceFile, DeckBuildError> {
    let mut source_aliases: BTreeMap<String, String> = BTreeMap::new();
    for slide in &deck.slides {
        if let Some(source) = &slide.source {
            let import_path = deck_source_import_path(deck_rel_path, &source.path);
            if let Some(dir) = deck_dir {
                validate_source_on_disk(dir, source, &import_path)?;
            }
            source_aliases
                .entry(import_path)
                .or_insert_with(|| source_alias_for_slide(&slide.id));
        }
    }

    let deck_root = format!("{app_id}/{}/deck", deck.frontmatter.id);
    let plane_key = format!("{deck_root}/p");
    let mut items = vec![V2Item::UseTemplate {
        path: "presentation/slide-patterns".to_string(),
        alias: Some("slide".to_string()),
    }];
    for (import_path, alias) in &source_aliases {
        items.push(V2Item::UseTemplate {
            path: import_path.clone(),
            alias: Some(alias.clone()),
        });
    }

    let mut presentation_keywords = vec![
        string_kw("id", &deck.frontmatter.id),
        string_kw(
            "key",
            &format!(
                "{}@{}",
                deck.frontmatter.id,
                deck_assembly_path(deck_rel_path)
            ),
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
        append_slide_items(&mut items, &deck_root, deck_rel_path, slide, &source_aliases);
    }
    Ok(V2SourceFile { items })
}

fn validate_source_on_disk(
    deck_dir: &Path,
    source: &DeckSource,
    import_path: &str,
) -> Result<(), DeckBuildError> {
    let file_path = deck_dir.join(&source.path);
    let source_text = std::fs::read_to_string(&file_path).map_err(|error| DeckBuildError {
        line: source.line,
        message: format!(
            "[deck_source_missing] `@source({}#{})` file not found at `{}`: {error}",
            source.path,
            source.fragment,
            file_path.display()
        ),
    })?;
    let parsed = parse_v2_source(&source_text).map_err(|error| DeckBuildError {
        line: source.line,
        message: format!(
            "[deck_source_parse] `@source({}#{})` failed to parse: {error}",
            source.path, source.fragment
        ),
    })?;
    for item in &parsed.items {
        if let V2Item::TopLevel { name, .. } = item {
            if FORBIDDEN_SOURCE_ROOTS.contains(&name.as_str()) {
                return Err(DeckBuildError {
                    line: source.line,
                    message: format!(
                        "[deck_source_root_forbidden] `@source({}#{})` must not declare `{name}`",
                        source.path, source.fragment
                    ),
                });
            }
        }
    }
    let has_fragment = parsed.items.iter().any(|item| matches!(
        item,
        V2Item::TemplateDecl { name, .. } if name == &source.fragment
    ));
    if !has_fragment {
        return Err(DeckBuildError {
            line: source.line,
            message: format!(
                "[deck_source_fragment_missing] `@source({}#{})` has no template `{fragment}` (import `{import_path}`)",
                source.path,
                source.fragment,
                fragment = source.fragment,
            ),
        });
    }
    Ok(())
}

fn append_slide_items(
    items: &mut Vec<V2Item>,
    deck_root: &str,
    deck_rel_path: &str,
    slide: &DeckSlide,
    source_aliases: &BTreeMap<String, String>,
) {
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

    let blocks = if let Some(source) = &slide.source {
        let import_path = deck_source_import_path(deck_rel_path, &source.path);
        let alias = source_aliases
            .get(&import_path)
            .cloned()
            .unwrap_or_else(|| source_alias_for_slide(&slide.id));
        V2Expr::Call {
            path: vec![alias, source.fragment.clone()],
            args: CallArgs::empty(),
        }
    } else {
        V2Expr::List(
            slide
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
                .collect(),
        )
    };
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
            ("blocks".to_string(), blocks),
        ],
    ));
}

fn deck_source_import_path(deck_rel_path: &str, source_path: &str) -> String {
    let normalized = deck_rel_path.replace('\\', "/");
    let without_src = normalized
        .strip_prefix("src/")
        .unwrap_or(normalized.as_str());
    let stage_dir = without_src
        .rsplit_once('/')
        .map(|(dir, _)| dir)
        .unwrap_or("");
    let custom = source_path
        .trim_end_matches(".mei")
        .trim_start_matches('/');
    if stage_dir.is_empty() {
        custom.to_string()
    } else {
        format!("{stage_dir}/{custom}")
    }
}

fn source_alias_for_slide(slide_id: &str) -> String {
    let mut alias = String::from("src_");
    for ch in slide_id.chars() {
        if ch.is_ascii_alphanumeric() {
            alias.push(ch);
        } else {
            alias.push('_');
        }
    }
    alias
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
    fn builds_ordered_v2_items_without_authored_script() {
        let deck = parse_deck_source(
            r#"---
id: intro
title: Intro
default_for_stage: true
---
# First {#s1}
@template(full_bleed)
## hero {#vp_hero}
**Hero**
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
            && value
                == &V2Expr::String("intro@src/presentation/intro/intro.deck.mdx".to_string())));
        assert!(presentation
            .keywords
            .iter()
            .all(|(key, _)| key != "default_script"));
    }

    #[test]
    fn lowers_source_to_template_call_blocks() {
        let deck = parse_deck_source(
            r#"---
id: intro
title: Intro
---
# First {#s1}
@template(full_bleed)
@source(custom/slide.mei#hero_blocks)
"#,
        )
        .expect("source parses");
        let v2 = deck_to_v2("demo", "presentation/intro/intro.deck.mdx", &deck).expect("v2");
        assert!(v2.items.iter().any(|item| matches!(
            item,
            V2Item::UseTemplate { path, alias: Some(alias) }
                if path == "presentation/intro/custom/slide" && alias == "src_s1"
        )));
        let panel = v2
            .items
            .iter()
            .find_map(|item| match item {
                V2Item::TopLevel { name, args } if name == "content_panel" => Some(args),
                _ => None,
            })
            .expect("content_panel");
        let blocks = panel
            .keywords
            .iter()
            .find_map(|(key, value)| (key == "blocks").then_some(value))
            .expect("blocks");
        assert_eq!(
            blocks,
            &V2Expr::Call {
                path: vec!["src_s1".to_string(), "hero_blocks".to_string()],
                args: CallArgs::empty(),
            }
        );
    }

    #[test]
    fn import_path_joins_stage_dir_and_custom() {
        assert_eq!(
            deck_source_import_path(
                "src/presentation/intro/intro.deck.mdx",
                "custom/graph.mei"
            ),
            "presentation/intro/custom/graph"
        );
        assert_eq!(
            deck_source_import_path("presentation/intro/intro.deck.mdx", "custom/graph.mei"),
            "presentation/intro/custom/graph"
        );
    }
}
