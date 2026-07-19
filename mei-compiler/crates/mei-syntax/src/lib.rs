pub mod admin_mdx;
mod ast;
pub mod deck;
pub mod narration;
mod parse;
mod policy;
pub mod stage_mdx;
pub mod stage_program_discover;
pub mod v2;

pub use admin_mdx::{
    parse_admin_mdx_file, parse_admin_mdx_source, AdminMdxDocument, AdminMdxError, AdminMdxFill,
    AdminMdxFrontmatter, ADMIN_API_VERSION, ADMIN_API_VERSION_UNSUPPORTED,
    ADMIN_FRONTMATTER_FIELD_FORBIDDEN, ADMIN_IDENTITY_REDECLARATION_FORBIDDEN,
    ADMIN_MDX_FORBIDDEN_PRESENTATION, ADMIN_MDX_JSX_FORBIDDEN, ADMIN_MDX_JS_FORBIDDEN,
    ADMIN_MDX_PARSE, ADMIN_SCENE_PHYSICAL_PATH_FORBIDDEN, ADMIN_SCENE_ROOT_DUPLICATE,
    ADMIN_SCENE_ROOT_MISSING,
};
pub use ast::*;
pub use deck::{
    parse_deck_source, parse_deck_source_file, DeckFile, DeckFrontmatter, DeckMarkdown,
    DeckParseError, DeckSlide, DeckSlot, DeckSource, DeckStep, DECK_NARRATION_DIRECTIVE_FORBIDDEN,
};
pub use narration::{
    parse_narration_track_file, parse_narration_track_source, NarrationCue as NarrationTrackCue,
    NarrationParseError, NarrationTiming, NarrationTrackFile, NarrationTrackFrontmatter,
    NARRATION_CUE_ID_DUPLICATE, NARRATION_PARSE, NARRATION_SOURCE_ANCHOR_MISSING,
    NARRATION_SOURCE_PATH_INVALID, NARRATION_TRACK_ID_DUPLICATE,
};
pub use parse::{parse_source, parse_source_file, ParseError};
pub use policy::{forbidden_authoring_tokens, validate_authoring_policy};
pub use stage_mdx::{
    parse_cockpit_stage_file, parse_cockpit_stage_source, CockpitStageFile, StageMdxError,
};
pub use stage_program_discover::{
    discover_program_for_stage, discover_stage_programs, scene_use_to_target,
    DiscoveredStageProgram, StageProgramProfile,
};
pub use v2::{
    parse_v2_source, parse_v2_source_file, parse_world_v2_source, V2ParseError, V2SourceFile,
};
