//! Built-in item-based mastery training: Anki-shaped phase + due, simplified SM-2.
//!
//! Progress is stored under `{workspace}/deploy/runtime/training/{app_id}/{username}/`.

mod model;
mod packs;
mod paths;
mod queue;
mod sm2;
mod store;
mod wubi;

pub use model::{
    ItemPhase, LadderStage, LearnerItemState, LearnerStateFile, MetaFile, Rating, ReviewLogEntry,
    LEARNER_SCHEMA_VERSION, SCHEDULER_ID,
};
pub use packs::{load_pack_catalog, PackCatalog, PackDef, UnlockRule};
pub use paths::{sanitize_username, training_learner_dir, training_root};
pub use queue::{
    next_item, review_item, session_summary, NextItem, NextRequest, PracticeIntent, QueueEmpty,
    ReviewRequest, ReviewResult, SessionSummary, TrainingMode, T_FLUENT_MS, T_LOOSE_MS,
};
pub use sm2::now_millis;
pub use store::{LearnerStore, StoreError};
pub use wubi::{
    level1_brief_key, load_wubi_catalog, WubiCatalog, WubiCharItem, WubiRadicalItem,
};
