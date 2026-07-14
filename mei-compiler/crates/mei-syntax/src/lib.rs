mod ast;
pub mod deck;
mod parse;
mod policy;
pub mod stage_mdx;
pub mod stage_program_discover;
pub mod v2;

pub use ast::*;
pub use deck::{
    parse_deck_source, parse_deck_source_file, DeckFile, DeckFrontmatter, DeckMarkdown,
    DeckParseError, DeckSlide, DeckSlot, DeckSource, DeckStep,
};
pub use stage_mdx::{
    parse_cockpit_stage_file, parse_cockpit_stage_source, CockpitStageFile, StageMdxError,
};
pub use stage_program_discover::{
    discover_program_for_stage, discover_stage_programs, scene_use_to_target,
    DiscoveredStageProgram, StageProgramProfile,
};
pub use parse::{parse_source, parse_source_file, ParseError};
pub use policy::{forbidden_authoring_tokens, validate_authoring_policy};
pub use v2::{
    parse_v2_source, parse_v2_source_file, parse_world_v2_source, V2ParseError, V2SourceFile,
};
