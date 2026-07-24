use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{
    ItemPhase, LearnerItemState, LearnerStateFile, Rating, ReviewLogEntry, SCHEDULER_ID,
};
use crate::sm2::{apply_rating, introduce_into_learning};
use crate::store::LearnerStore;
use crate::wubi::WubiCatalog;

pub const DEFAULT_NEW_CAP: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingMode {
    CharToCode,
    RadicalKey,
}

impl TrainingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CharToCode => "char_to_code",
            Self::RadicalKey => "radical_key",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "char_to_code" | "char-to-code" => Some(Self::CharToCode),
            "radical_key" | "radical-key" => Some(Self::RadicalKey),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextRequest {
    pub mode: TrainingMode,
    #[serde(default)]
    pub show_hint: bool,
    #[serde(default)]
    pub open_d2: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub mode: TrainingMode,
    pub item_id: String,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub correct: Option<bool>,
    #[serde(default)]
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub learner_id: String,
    pub due_count: u32,
    pub learning_due: u32,
    pub review_due: u32,
    pub introduced_count: u32,
    pub new_remaining_today: u32,
    pub new_cap: u32,
    pub char_pool: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextItem {
    pub item_id: String,
    pub mode: TrainingMode,
    pub phase: ItemPhase,
    pub payload: Value,
    pub show_hint: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEmpty {
    pub empty: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub correct: bool,
    pub expected: Option<String>,
    pub rating: Rating,
    pub phase_after: ItemPhase,
    pub due_at: i64,
    pub item_id: String,
}

pub fn session_summary(
    store: &LearnerStore,
    catalog: &WubiCatalog,
    now_ms: i64,
) -> anyhow::Result<SessionSummary> {
    store.with_lock(|| {
        let mut state = store.load_state()?;
        roll_new_day(&mut state, now_ms);
        let (learning_due, review_due) = count_due(&state, catalog, now_ms);
        Ok(SessionSummary {
            learner_id: store.learner_id.clone(),
            due_count: learning_due + review_due,
            learning_due,
            review_due,
            introduced_count: state.items.values().filter(|s| s.introduced).count() as u32,
            new_remaining_today: DEFAULT_NEW_CAP.saturating_sub(state.new_introduced_today),
            new_cap: DEFAULT_NEW_CAP,
            char_pool: state.char_pool.clone(),
        })
    })
}

pub fn next_item(
    store: &LearnerStore,
    catalog: &WubiCatalog,
    req: &NextRequest,
    now_ms: i64,
) -> anyhow::Result<Result<NextItem, QueueEmpty>> {
    store.with_lock(|| {
        let mut state = store.load_state()?;
        roll_new_day(&mut state, now_ms);
        if req.open_d2 {
            state.char_pool = "d2".to_string();
        }
        auto_open_d2_if_ready(&mut state, catalog);

        let pool_ids = catalog.item_ids_for_mode(req.mode, &state.char_pool);
        if let Some(item_id) = pick_due(&state, &pool_ids, now_ms, true) {
            let phase = state
                .items
                .get(&item_id)
                .map(|s| s.phase)
                .unwrap_or(ItemPhase::Learning);
            let payload = catalog.payload_for(&item_id, req.show_hint);
            store.save_state(&state)?;
            return Ok(Ok(NextItem {
                item_id,
                mode: req.mode,
                phase,
                payload,
                show_hint: req.show_hint,
            }));
        }
        if let Some(item_id) = pick_due(&state, &pool_ids, now_ms, false) {
            let phase = state
                .items
                .get(&item_id)
                .map(|s| s.phase)
                .unwrap_or(ItemPhase::Review);
            let payload = catalog.payload_for(&item_id, req.show_hint);
            store.save_state(&state)?;
            return Ok(Ok(NextItem {
                item_id,
                mode: req.mode,
                phase,
                payload,
                show_hint: req.show_hint,
            }));
        }

        if state.new_introduced_today >= DEFAULT_NEW_CAP {
            store.save_state(&state)?;
            return Ok(Err(QueueEmpty {
                empty: true,
                reason: "daily_new_cap_reached".into(),
            }));
        }

        if let Some(item_id) = pick_new(&state, &pool_ids) {
            let entry = state
                .items
                .entry(item_id.clone())
                .or_insert_with(LearnerItemState::fresh_unintroduced);
            introduce_into_learning(entry, now_ms);
            state.new_introduced_today = state.new_introduced_today.saturating_add(1);
            let payload = catalog.payload_for(&item_id, req.show_hint);
            store.save_state(&state)?;
            return Ok(Ok(NextItem {
                item_id,
                mode: req.mode,
                phase: ItemPhase::Learning,
                payload,
                show_hint: req.show_hint,
            }));
        }

        store.save_state(&state)?;
        Ok(Err(QueueEmpty {
            empty: true,
            reason: "no_more_items".into(),
        }))
    })
}

pub fn review_item(
    store: &LearnerStore,
    catalog: &WubiCatalog,
    req: &ReviewRequest,
    now_ms: i64,
) -> anyhow::Result<ReviewResult> {
    store.with_lock(|| {
        let mut state = store.load_state()?;
        roll_new_day(&mut state, now_ms);

        let (correct, expected) = catalog.judge(&req.item_id, req.answer.as_deref(), req.correct);
        let rating = if correct {
            Rating::Good
        } else {
            Rating::Again
        };

        let entry = state
            .items
            .entry(req.item_id.clone())
            .or_insert_with(LearnerItemState::fresh_unintroduced);
        if !entry.introduced {
            introduce_into_learning(entry, now_ms);
            state.new_introduced_today = state.new_introduced_today.saturating_add(1);
        }
        let phase_before = entry.phase;
        let due_before = entry.due_at;
        apply_rating(entry, rating, now_ms);
        let phase_after = entry.phase;
        let due_after = entry.due_at;

        let log = ReviewLogEntry {
            ts: Utc::now(),
            item_id: req.item_id.clone(),
            learner_id: store.learner_id.clone(),
            rating,
            correct,
            latency_ms: req.latency_ms,
            phase_before,
            phase_after,
            due_before,
            due_after,
            scheduler: SCHEDULER_ID.to_string(),
            mode: Some(req.mode.as_str().to_string()),
        };
        store.append_log(&log)?;
        store.save_state(&state)?;

        Ok(ReviewResult {
            correct,
            expected,
            rating,
            phase_after,
            due_at: due_after,
            item_id: req.item_id.clone(),
        })
    })
}

fn roll_new_day(state: &mut LearnerStateFile, now_ms: i64) {
    let day = millis_to_utc_day(now_ms);
    if state.new_day != day {
        state.new_day = day;
        state.new_introduced_today = 0;
    }
}

fn millis_to_utc_day(now_ms: i64) -> String {
    use chrono::{TimeZone, Utc};
    Utc.timestamp_millis_opt(now_ms)
        .single()
        .unwrap_or_else(Utc::now)
        .format("%Y-%m-%d")
        .to_string()
}

fn count_due(state: &LearnerStateFile, catalog: &WubiCatalog, now_ms: i64) -> (u32, u32) {
    let mut learning = 0u32;
    let mut review = 0u32;
    for (id, s) in &state.items {
        if s.suspended || !s.introduced || s.due_at > now_ms {
            continue;
        }
        if !catalog.contains(id) {
            continue;
        }
        match s.phase {
            ItemPhase::Learning | ItemPhase::Relearning | ItemPhase::New => {
                learning = learning.saturating_add(1)
            }
            ItemPhase::Review => review = review.saturating_add(1),
        }
    }
    (learning, review)
}

fn pick_due(
    state: &LearnerStateFile,
    pool_ids: &[String],
    now_ms: i64,
    learning_only: bool,
) -> Option<String> {
    let mut best: Option<(i64, String)> = None;
    for id in pool_ids {
        let Some(s) = state.items.get(id) else {
            continue;
        };
        if s.suspended || !s.introduced || s.due_at > now_ms {
            continue;
        }
        let is_learning = matches!(
            s.phase,
            ItemPhase::Learning | ItemPhase::Relearning | ItemPhase::New
        );
        if learning_only && !is_learning {
            continue;
        }
        if !learning_only && is_learning {
            continue;
        }
        match &best {
            None => best = Some((s.due_at, id.clone())),
            Some((due, _)) if s.due_at < *due => best = Some((s.due_at, id.clone())),
            _ => {}
        }
    }
    best.map(|(_, id)| id)
}

fn pick_new(state: &LearnerStateFile, pool_ids: &[String]) -> Option<String> {
    for id in pool_ids {
        let introduced = state
            .items
            .get(id)
            .map(|s| s.introduced)
            .unwrap_or(false);
        if !introduced {
            return Some(id.clone());
        }
    }
    None
}

fn auto_open_d2_if_ready(state: &mut LearnerStateFile, catalog: &WubiCatalog) {
    if state.char_pool == "d2" {
        return;
    }
    let d1_ids = catalog.item_ids_for_mode(TrainingMode::CharToCode, "d1");
    if d1_ids.is_empty() {
        return;
    }
    let introduced = d1_ids
        .iter()
        .filter(|id| state.items.get(*id).map(|s| s.introduced).unwrap_or(false))
        .count();
    if introduced * 100 / d1_ids.len() >= 50 {
        state.char_pool = "d2".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sm2::now_millis;
    use crate::wubi::{WubiCharItem, WubiRadicalItem};
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn tiny_catalog() -> WubiCatalog {
        let mut chars = BTreeMap::new();
        chars.insert(
            "国".into(),
            WubiCharItem {
                ch: "国".into(),
                code: "lgyi".into(),
                tier: "d1".into(),
            },
        );
        chars.insert(
            "中".into(),
            WubiCharItem {
                ch: "中".into(),
                code: "k".into(),
                tier: "d1".into(),
            },
        );
        let mut radicals = BTreeMap::new();
        radicals.insert(
            "G".into(),
            WubiRadicalItem {
                key: "G".into(),
                mnemonic: "王旁青头五一提".into(),
                examples: vec!["王".into()],
            },
        );
        WubiCatalog { chars, radicals }
    }

    #[test]
    fn next_introduces_then_reviews() {
        let dir = tempdir().unwrap();
        let store = LearnerStore::open(dir.path(), "wubi", "bob");
        let catalog = tiny_catalog();
        let now = now_millis();
        let next = next_item(
            &store,
            &catalog,
            &NextRequest {
                mode: TrainingMode::CharToCode,
                show_hint: false,
                open_d2: false,
            },
            now,
        )
        .unwrap()
        .unwrap();
        assert!(next.item_id.starts_with("char:"));
        let review = review_item(
            &store,
            &catalog,
            &ReviewRequest {
                mode: TrainingMode::CharToCode,
                item_id: next.item_id.clone(),
                answer: Some("wrong".into()),
                correct: None,
                latency_ms: 500,
            },
            now,
        )
        .unwrap();
        assert!(!review.correct);
    }
}
