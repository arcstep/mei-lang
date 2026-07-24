//! Built-in item-based mastery training: Anki-shaped phase + due, simplified SM-2.
//!
//! Progress is stored under `{workspace}/deploy/runtime/training/{app_id}/{username}/`.

mod model;
mod paths;
mod queue;
mod sm2;
mod store;
mod wubi;

pub use model::{
    ItemPhase, LearnerItemState, LearnerStateFile, MetaFile, Rating, ReviewLogEntry, SCHEDULER_ID,
};
pub use paths::{sanitize_username, training_learner_dir, training_root};
pub use queue::{
    next_item, review_item, session_summary, NextItem, NextRequest, QueueEmpty, ReviewRequest,
    ReviewResult, SessionSummary, TrainingMode,
};
pub use sm2::now_millis;
pub use store::{LearnerStore, StoreError};
pub use wubi::{load_wubi_catalog, WubiCatalog, WubiCharItem, WubiRadicalItem};
