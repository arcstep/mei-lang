//! Profile-aware Stage MDX front-end (Phase 4).
//!
//! - [`core`]: shared frontmatter / Markdown safety / directive helpers
//! - [`cockpit`]: Cockpit `*.stage.mdx` profile
//!
//! Slides continue to use [`crate::deck`] as a thin specialization over the same safety rules.

pub mod cockpit;
pub mod core;

pub use cockpit::{
    parse_cockpit_stage_file, parse_cockpit_stage_source, scene_id_from_use, CockpitFill,
    CockpitFrontmatter, CockpitNarrationStep, CockpitStageFile,
};
pub use core::{
    check_markdown_line, codes, find_frontmatter_end, looks_like_private_scene_path,
    markdown_from_lines, parse_directive_arg, parse_frontmatter_map, parse_heading,
    parse_named_directive_args, unquote, validate_id_token, MarkdownForbidden, StageMarkdown,
    StageMdxError,
};
