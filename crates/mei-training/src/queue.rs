use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::model::{
    ItemPhase, LadderStage, LearnerItemState, LearnerStateFile, Rating, ReviewLogEntry,
    LEARNER_SCHEMA_VERSION, SCHEDULER_ID,
};
use crate::packs::UnlockRule;
use crate::sm2::{apply_rating, introduce_into_learning};
use crate::store::LearnerStore;
use crate::wubi::WubiCatalog;

pub const DEFAULT_NEW_CAP: u32 = 20;
pub const T_LOOSE_MS: u64 = 3000;
pub const T_FLUENT_MS: u64 = 1600;

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

/// Session practice intent (difficulty policy). Orthogonal to Pack scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PracticeIntent {
    /// Learn / 学懂 — no time target; prefer new & lapses.
    Learn,
    /// Steady / 练稳 — loose threshold; prefer not-yet-L2.
    #[default]
    Steady,
    /// Speed / 练速 — fluent threshold; only items already ≥ L2.
    Speed,
}

impl PracticeIntent {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "learn" | "学懂" => Some(Self::Learn),
            "steady" | "练稳" => Some(Self::Steady),
            "speed" | "练速" => Some(Self::Speed),
            "" => Some(Self::Steady),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Learn => "learn",
            Self::Steady => "steady",
            Self::Speed => "speed",
        }
    }

    pub fn default_time_target_ms(self) -> Option<u64> {
        match self {
            Self::Learn => None,
            Self::Steady => Some(T_LOOSE_MS),
            Self::Speed => Some(T_FLUENT_MS),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextRequest {
    pub mode: TrainingMode,
    #[serde(default)]
    pub show_hint: bool,
    /// Legacy; ignored. Kept so old clients do not fail deserialize.
    #[serde(default)]
    pub open_d2: bool,
    #[serde(default)]
    pub intent: PracticeIntent,
    /// Optional matrix focus pack (must be unlocked).
    #[serde(default)]
    pub pack_id: Option<String>,
    /// Optional matrix focus ladder (`l1`/`l2`/`l3`).
    #[serde(default)]
    pub target_ladder: Option<LadderStage>,
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
    /// Optional mastery threshold (ms). When unset, derived from `intent`.
    #[serde(default)]
    pub time_target_ms: Option<u64>,
    #[serde(default)]
    pub intent: PracticeIntent,
    #[serde(default)]
    pub pack_id: Option<String>,
    #[serde(default)]
    pub target_ladder: Option<LadderStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusRef {
    pub pack_id: String,
    pub target_ladder: LadderStage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixCell {
    pub pct: u32,
    pub passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixRow {
    pub pack_id: String,
    pub title: String,
    pub unlocked: bool,
    pub total: u32,
    pub introduced: u32,
    pub cells: BTreeMap<String, MatrixCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PracticeMatrix {
    pub columns: Vec<String>,
    pub rows: Vec<MatrixRow>,
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
    /// Deprecated mirror of open packs for older UI; prefer `open_packs`.
    pub char_pool: String,
    #[serde(default)]
    pub open_packs: Vec<String>,
    #[serde(default)]
    pub active_pack_id: String,
    #[serde(default)]
    pub active_pack_title: String,
    #[serde(default)]
    pub active_pack_introduced: u32,
    #[serde(default)]
    pub active_pack_total: u32,
    #[serde(default)]
    pub pack_l2_pct: u32,
    #[serde(default)]
    pub pack_l3_pct: u32,
    #[serde(default)]
    pub can_unlock_next: bool,
    #[serde(default)]
    pub next_pack_id: Option<String>,
    #[serde(default)]
    pub focus: Option<FocusRef>,
    #[serde(default)]
    pub recommended_focus: Option<FocusRef>,
    #[serde(default)]
    pub matrix: Option<PracticeMatrix>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextItem {
    pub item_id: String,
    pub mode: TrainingMode,
    pub phase: ItemPhase,
    pub payload: Value,
    pub show_hint: bool,
    #[serde(default)]
    pub ladder_stage: LadderStage,
    #[serde(default)]
    pub intent: PracticeIntent,
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
    #[serde(default)]
    pub ladder_stage: LadderStage,
}

pub fn session_summary(
    store: &LearnerStore,
    catalog: &WubiCatalog,
    now_ms: i64,
) -> anyhow::Result<SessionSummary> {
    store.with_lock(|| {
        let mut state = store.load_state()?;
        migrate_and_unlock(&mut state, catalog);
        roll_new_day(&mut state, now_ms);
        let (learning_due, review_due) = count_due(&state, catalog, now_ms);
        let active = if state.active_pack_id.is_empty() {
            state
                .open_packs
                .last()
                .cloned()
                .unwrap_or_else(|| "pack-a".into())
        } else {
            state.active_pack_id.clone()
        };
        let (intro, total) = pack_progress(&state, catalog, &active);
        let (l2, l3) = pack_ladder_pct(&state, catalog, &active);
        let next = catalog.packs.next_locked_after(&state.open_packs);
        let can = next
            .map(|p| unlock_satisfied(&state, catalog, p))
            .unwrap_or(false);
        let title = catalog
            .packs
            .get(&active)
            .map(|p| p.title.clone())
            .unwrap_or_default();
        let focus = FocusRef {
            pack_id: state.focus_pack_id.clone(),
            target_ladder: state.focus_target,
        };
        let recommended_focus = recommended_focus(&state, catalog);
        let matrix = build_matrix(&state, catalog);
        store.save_state(&state)?;
        Ok(SessionSummary {
            learner_id: store.learner_id.clone(),
            due_count: learning_due + review_due,
            learning_due,
            review_due,
            introduced_count: state.items.values().filter(|s| s.introduced).count() as u32,
            new_remaining_today: DEFAULT_NEW_CAP.saturating_sub(state.new_introduced_today),
            new_cap: DEFAULT_NEW_CAP,
            char_pool: state.open_packs.join(","),
            open_packs: state.open_packs.clone(),
            active_pack_id: active,
            active_pack_title: title,
            active_pack_introduced: intro,
            active_pack_total: total,
            pack_l2_pct: l2,
            pack_l3_pct: l3,
            can_unlock_next: can,
            next_pack_id: next.map(|p| p.id.clone()),
            focus: Some(focus),
            recommended_focus: Some(recommended_focus),
            matrix: Some(matrix),
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
        migrate_and_unlock(&mut state, catalog);
        roll_new_day(&mut state, now_ms);
        try_auto_unlock(&mut state, catalog);
        let intent = resolve_focus(
            &mut state,
            catalog,
            req.pack_id.as_deref(),
            req.target_ladder,
            req.intent,
            req.pack_id.is_some() || req.target_ladder.is_some(),
        );
        let mut focused = req.clone();
        focused.intent = intent;

        let due_pool = due_pool_ids(catalog, focused.mode);
        let new_pool = new_pool_ids(catalog, &state, focused.mode);
        let focus_pack = state.focus_pack_id.clone();

        if let Some(item_id) =
            pick_due_filtered(&state, catalog, &due_pool, now_ms, true, intent, &focus_pack)
        {
            return finish_next(&store, &mut state, catalog, &focused, item_id);
        }
        if let Some(item_id) =
            pick_due_filtered(&state, catalog, &due_pool, now_ms, false, intent, &focus_pack)
        {
            return finish_next(&store, &mut state, catalog, &focused, item_id);
        }

        if intent == PracticeIntent::Speed {
            store.save_state(&state)?;
            return Ok(Err(QueueEmpty {
                empty: true,
                reason: "no_l2_items_for_speed".into(),
            }));
        }

        if state.new_introduced_today >= DEFAULT_NEW_CAP {
            store.save_state(&state)?;
            return Ok(Err(QueueEmpty {
                empty: true,
                reason: "daily_new_cap_reached".into(),
            }));
        }

        if let Some(item_id) = pick_new(&state, &new_pool, store.learner_id.as_str()) {
            let entry = state
                .items
                .entry(item_id.clone())
                .or_insert_with(LearnerItemState::fresh_unintroduced);
            introduce_into_learning(entry, now_ms);
            state.new_introduced_today = state.new_introduced_today.saturating_add(1);
            return finish_next(&store, &mut state, catalog, &focused, item_id);
        }

        store.save_state(&state)?;
        Ok(Err(QueueEmpty {
            empty: true,
            reason: "no_more_items".into(),
        }))
    })
}

fn finish_next(
    store: &LearnerStore,
    state: &mut LearnerStateFile,
    catalog: &WubiCatalog,
    req: &NextRequest,
    item_id: String,
) -> anyhow::Result<Result<NextItem, QueueEmpty>> {
    let phase = state
        .items
        .get(&item_id)
        .map(|s| s.phase)
        .unwrap_or(ItemPhase::Learning);
    let ladder_stage = state
        .items
        .get(&item_id)
        .map(|s| s.ladder_stage)
        .unwrap_or(LadderStage::L0);
    let show_hint = req.show_hint || req.intent == PracticeIntent::Learn;
    let payload = catalog.payload_for(&item_id, show_hint);
    store.save_state(state)?;
    Ok(Ok(NextItem {
        item_id,
        mode: req.mode,
        phase,
        payload,
        show_hint,
        ladder_stage,
        intent: req.intent,
    }))
}

pub fn review_item(
    store: &LearnerStore,
    catalog: &WubiCatalog,
    req: &ReviewRequest,
    now_ms: i64,
) -> anyhow::Result<ReviewResult> {
    store.with_lock(|| {
        let mut state = store.load_state()?;
        migrate_and_unlock(&mut state, catalog);
        roll_new_day(&mut state, now_ms);
        let intent = resolve_focus(
            &mut state,
            catalog,
            req.pack_id.as_deref(),
            req.target_ladder,
            req.intent,
            req.pack_id.is_some() || req.target_ladder.is_some(),
        );

        let (correct, expected) = catalog.judge(&req.item_id, req.answer.as_deref(), req.correct);
        let time_target = req
            .time_target_ms
            .or_else(|| intent.default_time_target_ms());
        let rating = map_review_rating(correct, req.latency_ms, time_target);

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
        apply_ladder(entry, correct, rating, intent, time_target, req.latency_ms);
        let phase_after = entry.phase;
        let due_after = entry.due_at;
        let ladder_stage = entry.ladder_stage;

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
            intent: Some(intent.as_str().to_string()),
            ladder_after: Some(ladder_stage.as_str().to_string()),
        };
        store.append_log(&log)?;
        try_auto_unlock(&mut state, catalog);
        store.save_state(&state)?;

        Ok(ReviewResult {
            correct,
            expected,
            rating,
            phase_after,
            due_at: due_after,
            item_id: req.item_id.clone(),
            ladder_stage,
        })
    })
}

fn apply_ladder(
    entry: &mut LearnerItemState,
    correct: bool,
    rating: Rating,
    intent: PracticeIntent,
    time_target: Option<u64>,
    latency_ms: u64,
) {
    if !correct {
        return;
    }
    match intent {
        PracticeIntent::Learn => {
            if entry.ladder_stage.rank() < LadderStage::L1.rank() {
                entry.ladder_stage = LadderStage::L1;
            }
        }
        PracticeIntent::Steady => {
            if entry.ladder_stage.rank() < LadderStage::L1.rank() {
                entry.ladder_stage = LadderStage::L1;
            }
            let on_pace = match time_target.filter(|t| *t > 0) {
                Some(t) => latency_ms <= t,
                None => rating != Rating::Hard,
            };
            if on_pace && entry.ladder_stage.rank() < LadderStage::L2.rank() {
                entry.ladder_stage = LadderStage::L2;
            }
        }
        PracticeIntent::Speed => {
            if entry.ladder_stage.at_least(LadderStage::L2) {
                let on_pace = match time_target.filter(|t| *t > 0) {
                    Some(t) => latency_ms <= t,
                    None => rating != Rating::Hard,
                };
                if on_pace {
                    entry.ladder_stage = LadderStage::L3;
                }
            }
        }
    }
}

/// Map judge outcome + optional time target onto SM-2 rating.
pub fn map_review_rating(correct: bool, latency_ms: u64, time_target_ms: Option<u64>) -> Rating {
    if !correct {
        return Rating::Again;
    }
    match time_target_ms.filter(|ms| *ms > 0) {
        Some(target) if latency_ms > target => Rating::Hard,
        _ => Rating::Good,
    }
}

fn migrate_and_unlock(state: &mut LearnerStateFile, catalog: &WubiCatalog) {
    if state.open_packs.is_empty() {
        if state.char_pool == "d2" && !catalog.packs.packs.is_empty() {
            let mut open = vec!["pack-a".to_string()];
            open.extend(catalog.packs.all_b_pack_ids());
            state.open_packs = open;
        } else {
            state.open_packs = vec!["pack-a".into()];
        }
    }
    if state.active_pack_id.is_empty() {
        state.active_pack_id = state
            .open_packs
            .last()
            .cloned()
            .unwrap_or_else(|| "pack-a".into());
    }
    // Drop packs that no longer exist (except keep pack-a).
    state.open_packs.retain(|id| {
        id == "pack-a" || catalog.packs.packs.contains_key(id)
    });
    if state.open_packs.is_empty() {
        state.open_packs = vec!["pack-a".into()];
    }
    if state.focus_pack_id.is_empty()
        || !state.open_packs.iter().any(|p| p == &state.focus_pack_id)
    {
        state.focus_pack_id = state
            .open_packs
            .first()
            .cloned()
            .unwrap_or_else(|| "pack-a".into());
    }
    if !matches!(
        state.focus_target,
        LadderStage::L1 | LadderStage::L2 | LadderStage::L3
    ) {
        state.focus_target = LadderStage::L1;
    }
    state.schema_version = LEARNER_SCHEMA_VERSION;
}

/// Apply optional client focus; returns effective practice intent.
fn resolve_focus(
    state: &mut LearnerStateFile,
    catalog: &WubiCatalog,
    pack_id: Option<&str>,
    target_ladder: Option<LadderStage>,
    legacy_intent: PracticeIntent,
    client_sent_focus: bool,
) -> PracticeIntent {
    if let Some(pid) = pack_id.map(str::trim).filter(|s| !s.is_empty()) {
        if pack_is_unlocked(state, catalog, pid) {
            state.focus_pack_id = pid.to_string();
        }
        // Locked / unknown pack: ignore and keep persisted focus.
    }
    if let Some(target) = target_ladder {
        if matches!(
            target,
            LadderStage::L1 | LadderStage::L2 | LadderStage::L3
        ) {
            state.focus_target = target;
        }
    } else if !client_sent_focus {
        // Legacy clients send intent only — sync ladder column from intent.
        state.focus_target = ladder_from_intent(legacy_intent);
    }
    intent_from_ladder(state.focus_target)
}

fn pack_is_unlocked(state: &LearnerStateFile, catalog: &WubiCatalog, pack_id: &str) -> bool {
    if !state.open_packs.iter().any(|p| p == pack_id) {
        return false;
    }
    pack_id == "pack-a" || catalog.packs.packs.contains_key(pack_id)
}

fn intent_from_ladder(ladder: LadderStage) -> PracticeIntent {
    match ladder {
        LadderStage::L0 | LadderStage::L1 => PracticeIntent::Learn,
        LadderStage::L2 => PracticeIntent::Steady,
        LadderStage::L3 | LadderStage::L4 => PracticeIntent::Speed,
    }
}

fn ladder_from_intent(intent: PracticeIntent) -> LadderStage {
    match intent {
        PracticeIntent::Learn => LadderStage::L1,
        PracticeIntent::Steady => LadderStage::L2,
        PracticeIntent::Speed => LadderStage::L3,
    }
}

fn matrix_visible_pack_ids(catalog: &WubiCatalog) -> Vec<String> {
    if catalog.packs.order.is_empty() {
        return vec!["pack-a".into()];
    }
    catalog
        .packs
        .order
        .iter()
        .filter(|id| {
            catalog
                .packs
                .get(id)
                .map(|p| {
                    let tier = p.tier.to_ascii_uppercase();
                    tier != "C" && !id.starts_with("pack-c")
                })
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

fn recommend_path(catalog: &WubiCatalog) -> Vec<(String, LadderStage)> {
    let packs = matrix_visible_pack_ids(catalog);
    let mut path = Vec::new();
    for id in &packs {
        path.push((id.clone(), LadderStage::L1));
        path.push((id.clone(), LadderStage::L2));
    }
    for id in &packs {
        path.push((id.clone(), LadderStage::L3));
    }
    path
}

fn recommended_focus(state: &LearnerStateFile, catalog: &WubiCatalog) -> FocusRef {
    let path = recommend_path(catalog);
    for (pack_id, target) in &path {
        if !state.open_packs.iter().any(|p| p == pack_id) {
            continue;
        }
        if !cell_passed(state, catalog, pack_id, *target) {
            return FocusRef {
                pack_id: pack_id.clone(),
                target_ladder: *target,
            };
        }
    }
    // All unlocked cells passed: fall back to last L2 on path, else last cell.
    if let Some((pack_id, _)) = path.iter().rev().find(|(pack_id, t)| {
        *t == LadderStage::L2 && state.open_packs.iter().any(|p| p == pack_id)
    }) {
        return FocusRef {
            pack_id: pack_id.clone(),
            target_ladder: LadderStage::L2,
        };
    }
    path.last()
        .map(|(p, t)| FocusRef {
            pack_id: p.clone(),
            target_ladder: *t,
        })
        .unwrap_or(FocusRef {
            pack_id: "pack-a".into(),
            target_ladder: LadderStage::L1,
        })
}

fn cell_pct(state: &LearnerStateFile, catalog: &WubiCatalog, pack_id: &str, ladder: LadderStage) -> u32 {
    let ids = catalog.packs.char_item_ids(pack_id);
    if ids.is_empty() {
        return 0;
    }
    let reached = ids
        .iter()
        .filter(|id| {
            state
                .items
                .get(*id)
                .map(|s| s.introduced && s.ladder_stage.at_least(ladder))
                .unwrap_or(false)
        })
        .count();
    (reached * 100 / ids.len()) as u32
}

/// Pass thresholds per 16§6.1.
fn cell_passed(
    state: &LearnerStateFile,
    catalog: &WubiCatalog,
    pack_id: &str,
    ladder: LadderStage,
) -> bool {
    let ids = catalog.packs.char_item_ids(pack_id);
    let total = ids.len();
    if total == 0 {
        return false;
    }
    let intro = ids
        .iter()
        .filter(|id| state.items.get(*id).map(|s| s.introduced).unwrap_or(false))
        .count();
    let intro_pct = (intro * 100 / total) as u32;
    let at_ladder = ids
        .iter()
        .filter(|id| {
            state
                .items
                .get(*id)
                .map(|s| s.introduced && s.ladder_stage.at_least(ladder))
                .unwrap_or(false)
        })
        .count();
    let at_pct_total = (at_ladder * 100 / total) as u32;
    match ladder {
        LadderStage::L1 => intro_pct >= 90 && at_pct_total >= 80,
        LadderStage::L2 => intro_pct >= 100 && at_pct_total >= 60,
        LadderStage::L3 => {
            if intro == 0 {
                return false;
            }
            ((at_ladder * 100 / intro) as u32) >= 40
        }
        LadderStage::L0 | LadderStage::L4 => false,
    }
}

fn build_matrix(state: &LearnerStateFile, catalog: &WubiCatalog) -> PracticeMatrix {
    let columns = vec!["l1".into(), "l2".into(), "l3".into()];
    let ladders = [LadderStage::L1, LadderStage::L2, LadderStage::L3];
    let mut rows = Vec::new();
    for pack_id in matrix_visible_pack_ids(catalog) {
        let title = catalog
            .packs
            .get(&pack_id)
            .map(|p| p.title.clone())
            .unwrap_or_else(|| pack_id.clone());
        let unlocked = state.open_packs.iter().any(|p| p == &pack_id);
        let (introduced, total) = pack_progress(state, catalog, &pack_id);
        let mut cells = BTreeMap::new();
        for ladder in ladders {
            cells.insert(
                ladder.as_str().to_string(),
                MatrixCell {
                    pct: cell_pct(state, catalog, &pack_id, ladder),
                    passed: cell_passed(state, catalog, &pack_id, ladder),
                },
            );
        }
        rows.push(MatrixRow {
            pack_id,
            title,
            unlocked,
            total,
            introduced,
            cells,
        });
    }
    PracticeMatrix { columns, rows }
}

fn item_in_pack(catalog: &WubiCatalog, item_id: &str, pack_id: &str) -> bool {
    catalog
        .packs
        .char_item_ids(pack_id)
        .iter()
        .any(|id| id == item_id)
}

fn try_auto_unlock(state: &mut LearnerStateFile, catalog: &WubiCatalog) {
    loop {
        let Some(next) = catalog
            .packs
            .next_locked_after(&state.open_packs)
            .map(|p| p.id.clone())
        else {
            break;
        };
        let Some(pack) = catalog.packs.get(&next) else {
            break;
        };
        if !unlock_satisfied(state, catalog, pack) {
            break;
        }
        state.open_packs.push(next.clone());
        state.active_pack_id = next;
    }
}

fn unlock_satisfied(
    state: &LearnerStateFile,
    catalog: &WubiCatalog,
    pack: &crate::packs::PackDef,
) -> bool {
    match &pack.unlock {
        UnlockRule::DefaultOpen => true,
        UnlockRule::Manual => false,
        UnlockRule::PackMastery {
            requires,
            min_introduced_pct,
            min_l1_pct,
            min_l2_pct,
        } => {
            let ids = catalog.packs.char_item_ids(requires);
            if ids.is_empty() {
                return false;
            }
            let intro = ids
                .iter()
                .filter(|id| state.items.get(*id).map(|s| s.introduced).unwrap_or(false))
                .count();
            let intro_pct = (intro * 100 / ids.len()) as u32;
            if intro_pct < *min_introduced_pct {
                return false;
            }
            if *min_l1_pct > 0 {
                let l1 = ids
                    .iter()
                    .filter(|id| {
                        state
                            .items
                            .get(*id)
                            .map(|s| s.introduced && s.ladder_stage.at_least(LadderStage::L1))
                            .unwrap_or(false)
                    })
                    .count();
                if ((l1 * 100 / ids.len()) as u32) < *min_l1_pct {
                    return false;
                }
            }
            if *min_l2_pct > 0 {
                let l2 = ids
                    .iter()
                    .filter(|id| {
                        state
                            .items
                            .get(*id)
                            .map(|s| s.introduced && s.ladder_stage.at_least(LadderStage::L2))
                            .unwrap_or(false)
                    })
                    .count();
                if ((l2 * 100 / ids.len()) as u32) < *min_l2_pct {
                    return false;
                }
            }
            true
        }
    }
}

fn due_pool_ids(catalog: &WubiCatalog, mode: TrainingMode) -> Vec<String> {
    match mode {
        TrainingMode::RadicalKey => catalog.radical_item_ids(),
        // Due across all introduced chars in catalog (not limited to open packs).
        TrainingMode::CharToCode => catalog.all_char_item_ids(),
    }
}

fn new_pool_ids(catalog: &WubiCatalog, state: &LearnerStateFile, mode: TrainingMode) -> Vec<String> {
    match mode {
        TrainingMode::RadicalKey => catalog.radical_item_ids(),
        TrainingMode::CharToCode => {
            if catalog.packs.packs.is_empty() {
                // Fallback sample catalogs in unit tests.
                return catalog.item_ids_for_mode(mode, "all");
            }
            // New intros only from the focused pack (not all open packs).
            catalog.packs.char_item_ids(&state.focus_pack_id)
        }
    }
}

fn pack_progress(state: &LearnerStateFile, catalog: &WubiCatalog, pack_id: &str) -> (u32, u32) {
    let ids = catalog.packs.char_item_ids(pack_id);
    let total = ids.len() as u32;
    let intro = ids
        .iter()
        .filter(|id| state.items.get(*id).map(|s| s.introduced).unwrap_or(false))
        .count() as u32;
    (intro, total)
}

fn pack_ladder_pct(state: &LearnerStateFile, catalog: &WubiCatalog, pack_id: &str) -> (u32, u32) {
    let ids = catalog.packs.char_item_ids(pack_id);
    if ids.is_empty() {
        return (0, 0);
    }
    let intro: Vec<_> = ids
        .iter()
        .filter(|id| state.items.get(*id).map(|s| s.introduced).unwrap_or(false))
        .collect();
    if intro.is_empty() {
        return (0, 0);
    }
    let n = intro.len();
    let l2 = intro
        .iter()
        .filter(|id| {
            state
                .items
                .get(**id)
                .map(|s| s.ladder_stage.at_least(LadderStage::L2))
                .unwrap_or(false)
        })
        .count();
    let l3 = intro
        .iter()
        .filter(|id| {
            state
                .items
                .get(**id)
                .map(|s| s.ladder_stage.at_least(LadderStage::L3))
                .unwrap_or(false)
        })
        .count();
    ((l2 * 100 / n) as u32, (l3 * 100 / n) as u32)
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

fn pick_due_filtered(
    state: &LearnerStateFile,
    catalog: &WubiCatalog,
    pool_ids: &[String],
    now_ms: i64,
    learning_only: bool,
    intent: PracticeIntent,
    focus_pack: &str,
) -> Option<String> {
    let mut best: Option<(i64, i32, String)> = None;
    for id in pool_ids {
        let Some(s) = state.items.get(id) else {
            continue;
        };
        if s.suspended || !s.introduced || s.due_at > now_ms {
            continue;
        }
        if !intent_allows(s, intent) {
            continue;
        }
        // L3/speed: only items already ≥ L2 within the focused pack.
        if intent == PracticeIntent::Speed && !item_in_pack(catalog, id, focus_pack) {
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
        let same_pack = item_in_pack(catalog, id, focus_pack);
        let pref = intent_preference(s, intent) + if same_pack { 100 } else { 0 };
        match &best {
            None => best = Some((s.due_at, pref, id.clone())),
            Some((due, p, _)) if pref > *p || (pref == *p && s.due_at < *due) => {
                best = Some((s.due_at, pref, id.clone()))
            }
            _ => {}
        }
    }
    best.map(|(_, _, id)| id)
}

fn intent_allows(s: &LearnerItemState, intent: PracticeIntent) -> bool {
    match intent {
        PracticeIntent::Speed => s.ladder_stage.at_least(LadderStage::L2),
        PracticeIntent::Learn | PracticeIntent::Steady => true,
    }
}

fn intent_preference(s: &LearnerItemState, intent: PracticeIntent) -> i32 {
    match intent {
        PracticeIntent::Learn => {
            if matches!(s.phase, ItemPhase::Relearning) || s.lapses > 0 {
                2
            } else if !s.ladder_stage.at_least(LadderStage::L1) {
                1
            } else {
                0
            }
        }
        PracticeIntent::Steady => {
            if !s.ladder_stage.at_least(LadderStage::L2) {
                1
            } else {
                0
            }
        }
        PracticeIntent::Speed => 0,
    }
}

fn pick_new(state: &LearnerStateFile, pool_ids: &[String], learner_salt: &str) -> Option<String> {
    let candidates: Vec<&String> = pool_ids
        .iter()
        .filter(|id| {
            !state
                .items
                .get(*id)
                .map(|s| s.introduced)
                .unwrap_or(false)
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    // Per-draw entropy: wall clock (含时分秒/纳秒) + learner + 进程 + 单调计数，避免全员同序。
    let idx = (mix_entropy(learner_salt) as usize) % candidates.len();
    Some(candidates[idx].clone())
}

fn mix_entropy(learner_salt: &str) -> u64 {
    use chrono::{Datelike, Timelike, Utc};
    use std::sync::atomic::{AtomicU64, Ordering};

    static DRAW_SEQ: AtomicU64 = AtomicU64::new(1);

    let now = Utc::now();
    let mut h = 0xcbf2_9ce4_8422_2325u64; // FNV offset basis
    let mix = |h: &mut u64, v: u64| {
        *h ^= v;
        *h = h.wrapping_mul(0x100_0000_01b3); // FNV prime
    };

    mix(&mut h, now.timestamp() as u64);
    mix(&mut h, now.timestamp_subsec_nanos() as u64);
    mix(&mut h, now.year() as u64);
    mix(&mut h, now.month() as u64);
    mix(&mut h, now.day() as u64);
    mix(&mut h, now.hour() as u64);
    mix(&mut h, now.minute() as u64);
    mix(&mut h, now.second() as u64);
    mix(&mut h, DRAW_SEQ.fetch_add(1, Ordering::Relaxed));
    mix(&mut h, std::process::id() as u64);
    for b in learner_salt.as_bytes() {
        mix(&mut h, *b as u64);
    }
    // final avalanche (splitmix64)
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packs::{PackCatalog, PackDef, UnlockRule};
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
                tier: "pack".into(),
            },
        );
        chars.insert(
            "中".into(),
            WubiCharItem {
                ch: "中".into(),
                code: "k".into(),
                tier: "pack".into(),
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
        let mut packs = BTreeMap::new();
        packs.insert(
            "pack-a".into(),
            PackDef {
                id: "pack-a".into(),
                title: "规则基础字".into(),
                tier: "A".into(),
                order: 1,
                chars: vec!["国".into(), "中".into()],
                unlock: UnlockRule::DefaultOpen,
            },
        );
        WubiCatalog {
            chars,
            radicals,
            packs: PackCatalog {
                packs,
                order: vec!["pack-a".into()],
            },
        }
    }

    #[test]
    fn next_introduces_from_open_pack_only() {
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
                intent: PracticeIntent::Learn,
                pack_id: None,
                target_ladder: None,
            },
            now,
        )
        .unwrap()
        .unwrap();
        assert!(next.item_id == "char:国" || next.item_id == "char:中");
    }

    #[test]
    fn speed_skips_items_below_l2() {
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
                intent: PracticeIntent::Learn,
                pack_id: None,
                target_ladder: None,
            },
            now,
        )
        .unwrap()
        .unwrap();
        let _ = review_item(
            &store,
            &catalog,
            &ReviewRequest {
                mode: TrainingMode::CharToCode,
                item_id: next.item_id.clone(),
                answer: Some(
                    catalog
                        .chars
                        .get(next.item_id.strip_prefix("char:").unwrap())
                        .unwrap()
                        .code
                        .clone(),
                ),
                correct: None,
                latency_ms: 100,
                time_target_ms: None,
                intent: PracticeIntent::Learn,
                pack_id: None,
                target_ladder: None,
            },
            now,
        )
        .unwrap();
        let empty = next_item(
            &store,
            &catalog,
            &NextRequest {
                mode: TrainingMode::CharToCode,
                show_hint: false,
                open_d2: false,
                intent: PracticeIntent::Speed,
                pack_id: None,
                target_ladder: None,
            },
            now,
        )
        .unwrap();
        assert!(empty.is_err());
    }

    #[test]
    fn map_rating_respects_time_target() {
        assert_eq!(map_review_rating(false, 100, Some(3000)), Rating::Again);
        assert_eq!(map_review_rating(true, 100, None), Rating::Good);
        assert_eq!(map_review_rating(true, 100, Some(0)), Rating::Good);
        assert_eq!(map_review_rating(true, 2500, Some(3000)), Rating::Good);
        assert_eq!(map_review_rating(true, 3000, Some(3000)), Rating::Good);
        assert_eq!(map_review_rating(true, 3001, Some(3000)), Rating::Hard);
    }

    #[test]
    fn no_auto_open_without_mastery() {
        let mut packs = BTreeMap::new();
        packs.insert(
            "pack-a".into(),
            PackDef {
                id: "pack-a".into(),
                title: "A".into(),
                tier: "A".into(),
                order: 1,
                chars: vec!["国".into()],
                unlock: UnlockRule::DefaultOpen,
            },
        );
        packs.insert(
            "pack-b1".into(),
            PackDef {
                id: "pack-b1".into(),
                title: "B1".into(),
                tier: "B".into(),
                order: 2,
                chars: vec!["中".into()],
                unlock: UnlockRule::PackMastery {
                    requires: "pack-a".into(),
                    min_introduced_pct: 90,
                    min_l1_pct: 80,
                    min_l2_pct: 0,
                },
            },
        );
        let catalog = WubiCatalog {
            chars: BTreeMap::new(),
            radicals: BTreeMap::new(),
            packs: PackCatalog {
                packs,
                order: vec!["pack-a".into(), "pack-b1".into()],
            },
        };
        let mut state = LearnerStateFile::new();
        try_auto_unlock(&mut state, &catalog);
        assert_eq!(state.open_packs, vec!["pack-a".to_string()]);
    }

    fn two_pack_catalog() -> WubiCatalog {
        let mut chars = BTreeMap::new();
        chars.insert(
            "国".into(),
            WubiCharItem {
                ch: "国".into(),
                code: "lgyi".into(),
                tier: "pack".into(),
            },
        );
        chars.insert(
            "中".into(),
            WubiCharItem {
                ch: "中".into(),
                code: "k".into(),
                tier: "pack".into(),
            },
        );
        let mut packs = BTreeMap::new();
        packs.insert(
            "pack-a".into(),
            PackDef {
                id: "pack-a".into(),
                title: "规则基础字".into(),
                tier: "A".into(),
                order: 1,
                chars: vec!["国".into()],
                unlock: UnlockRule::DefaultOpen,
            },
        );
        packs.insert(
            "pack-b1".into(),
            PackDef {
                id: "pack-b1".into(),
                title: "B1".into(),
                tier: "B".into(),
                order: 2,
                chars: vec!["中".into()],
                unlock: UnlockRule::PackMastery {
                    requires: "pack-a".into(),
                    min_introduced_pct: 90,
                    min_l1_pct: 80,
                    min_l2_pct: 0,
                },
            },
        );
        WubiCatalog {
            chars,
            radicals: BTreeMap::new(),
            packs: PackCatalog {
                packs,
                order: vec!["pack-a".into(), "pack-b1".into()],
            },
        }
    }

    #[test]
    fn focus_locks_new_to_pack() {
        let dir = tempdir().unwrap();
        let store = LearnerStore::open(dir.path(), "wubi", "focus");
        let catalog = two_pack_catalog();
        // Unlock B1 manually and focus A — new must stay in A.
        {
            let mut state = store.load_state().unwrap();
            state.open_packs = vec!["pack-a".into(), "pack-b1".into()];
            store.save_state(&state).unwrap();
        }
        let now = now_millis();
        let next = next_item(
            &store,
            &catalog,
            &NextRequest {
                mode: TrainingMode::CharToCode,
                show_hint: true,
                open_d2: false,
                intent: PracticeIntent::Learn,
                pack_id: Some("pack-a".into()),
                target_ladder: Some(LadderStage::L1),
            },
            now,
        )
        .unwrap()
        .unwrap();
        assert_eq!(next.item_id, "char:国");
        assert_eq!(next.intent, PracticeIntent::Learn);
        let state = store.load_state().unwrap();
        assert_eq!(state.focus_pack_id, "pack-a");
        assert_eq!(state.focus_target, LadderStage::L1);
    }

    #[test]
    fn locked_pack_cannot_become_focus() {
        let dir = tempdir().unwrap();
        let store = LearnerStore::open(dir.path(), "wubi", "lock");
        let catalog = two_pack_catalog();
        let now = now_millis();
        let _ = next_item(
            &store,
            &catalog,
            &NextRequest {
                mode: TrainingMode::CharToCode,
                show_hint: false,
                open_d2: false,
                intent: PracticeIntent::Learn,
                pack_id: Some("pack-b1".into()),
                target_ladder: Some(LadderStage::L2),
            },
            now,
        )
        .unwrap();
        let state = store.load_state().unwrap();
        assert_eq!(state.focus_pack_id, "pack-a");
        assert_eq!(state.focus_target, LadderStage::L2);
    }

    #[test]
    fn recommended_skips_passed_cells() {
        let catalog = two_pack_catalog();
        let mut state = LearnerStateFile::new();
        // Mark pack-a fully passed for L1 (100% intro + L1).
        let mut item = LearnerItemState::fresh_unintroduced();
        item.introduced = true;
        item.ladder_stage = LadderStage::L1;
        state.items.insert("char:国".into(), item);
        let rec = recommended_focus(&state, &catalog);
        // A·L1 passed (1/1 intro ≥90%, 1/1 L1 ≥80%); next is A·L2.
        assert_eq!(rec.pack_id, "pack-a");
        assert_eq!(rec.target_ladder, LadderStage::L2);
    }

    #[test]
    fn speed_filters_to_focus_pack_l2() {
        let dir = tempdir().unwrap();
        let store = LearnerStore::open(dir.path(), "wubi", "spd");
        let catalog = two_pack_catalog();
        let now = now_millis();
        {
            let mut state = store.load_state().unwrap();
            state.open_packs = vec!["pack-a".into(), "pack-b1".into()];
            let mut a = LearnerItemState::fresh_unintroduced();
            a.introduced = true;
            a.ladder_stage = LadderStage::L2;
            a.phase = ItemPhase::Review;
            a.due_at = now - 1;
            state.items.insert("char:国".into(), a);
            let mut b = LearnerItemState::fresh_unintroduced();
            b.introduced = true;
            b.ladder_stage = LadderStage::L2;
            b.phase = ItemPhase::Review;
            b.due_at = now - 1;
            state.items.insert("char:中".into(), b);
            store.save_state(&state).unwrap();
        }
        let next = next_item(
            &store,
            &catalog,
            &NextRequest {
                mode: TrainingMode::CharToCode,
                show_hint: false,
                open_d2: false,
                intent: PracticeIntent::Speed,
                pack_id: Some("pack-a".into()),
                target_ladder: Some(LadderStage::L3),
            },
            now,
        )
        .unwrap()
        .unwrap();
        assert_eq!(next.item_id, "char:国");
        assert_eq!(next.intent, PracticeIntent::Speed);
    }

    #[test]
    fn pick_new_skips_introduced() {
        let mut state = LearnerStateFile::new();
        let mut a = LearnerItemState::fresh_unintroduced();
        a.introduced = true;
        state.items.insert("char:一".into(), a);
        let pool = vec!["char:一".into(), "char:中".into(), "char:国".into()];
        let picked = pick_new(&state, &pool, "dev").unwrap();
        assert_ne!(picked, "char:一");
        assert!(picked == "char:中" || picked == "char:国");
    }

    #[test]
    fn mix_entropy_varies_across_draws() {
        let a = mix_entropy("alice");
        let b = mix_entropy("alice");
        let c = mix_entropy("bob");
        // Sequential draws bump DRAW_SEQ; same salt still differs.
        assert_ne!(a, b);
        // Different learners almost always differ; allow rare collision but not all equal.
        assert!(a != c || b != c);
    }
}
