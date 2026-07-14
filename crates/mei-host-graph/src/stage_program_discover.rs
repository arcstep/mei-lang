//! Re-export Stage Program discovery from mei-syntax (0119).
//! Implementation lives in mei-syntax so mei-compiler can synthesize graph closure.

pub use mei_syntax::{
    discover_program_for_stage, discover_stage_programs, scene_use_to_target,
    DiscoveredStageProgram, StageProgramProfile,
};
