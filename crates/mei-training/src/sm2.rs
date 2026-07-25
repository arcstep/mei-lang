//! Simplified SM-2 scheduler (sm2-v0). Contract-level, not paper-faithful.

use chrono::{Duration, Utc};

use crate::model::{ItemPhase, LearnerItemState, Rating};

/// Learning / relearning short steps in minutes.
pub const LEARNING_STEPS_MIN: &[i64] = &[1, 10];

pub fn now_millis() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn introduce_into_learning(state: &mut LearnerItemState, now_ms: i64) {
    state.introduced = true;
    state.phase = ItemPhase::Learning;
    state.learning_step = 0;
    state.due_at = now_ms;
    if state.ease < 1.3 {
        state.ease = 2.5;
    }
}

/// Apply a rating and rewrite phase / due / ease.
pub fn apply_rating(state: &mut LearnerItemState, rating: Rating, now_ms: i64) {
    match state.phase {
        ItemPhase::New => {
            introduce_into_learning(state, now_ms);
            apply_rating(state, rating, now_ms);
        }
        ItemPhase::Learning | ItemPhase::Relearning => apply_learning(state, rating, now_ms),
        ItemPhase::Review => apply_review(state, rating, now_ms),
    }
}

fn apply_learning(state: &mut LearnerItemState, rating: Rating, now_ms: i64) {
    match rating {
        Rating::Again => {
            state.learning_step = 0;
            state.due_at = now_ms + minutes_ms(LEARNING_STEPS_MIN[0]);
        }
        Rating::Hard => {
            let step = state.learning_step as usize;
            let mins = LEARNING_STEPS_MIN
                .get(step)
                .copied()
                .unwrap_or(*LEARNING_STEPS_MIN.last().unwrap_or(&10));
            state.due_at = now_ms + minutes_ms(mins);
        }
        Rating::Good | Rating::Easy => {
            let next = state.learning_step + 1;
            if (next as usize) >= LEARNING_STEPS_MIN.len() {
                graduate_to_review(state, now_ms, rating == Rating::Easy);
            } else {
                state.learning_step = next;
                let mins = LEARNING_STEPS_MIN[next as usize];
                state.due_at = now_ms + minutes_ms(mins);
            }
        }
    }
}

fn graduate_to_review(state: &mut LearnerItemState, now_ms: i64, easy: bool) {
    state.phase = ItemPhase::Review;
    state.learning_step = 0;
    state.reps = state.reps.saturating_add(1);
    state.interval_days = if easy { 4.0 } else { 1.0 };
    if state.ease < 1.3 {
        state.ease = 2.5;
    }
    if easy {
        state.ease = (state.ease + 0.15).min(3.0);
    }
    state.due_at = now_ms + days_ms(state.interval_days);
}

fn apply_review(state: &mut LearnerItemState, rating: Rating, now_ms: i64) {
    match rating {
        Rating::Again => {
            state.lapses = state.lapses.saturating_add(1);
            state.phase = ItemPhase::Relearning;
            state.learning_step = 0;
            state.ease = (state.ease - 0.2).max(1.3);
            state.interval_days = 1.0_f64.max(state.interval_days * 0.5).min(1.0);
            state.due_at = now_ms + minutes_ms(LEARNING_STEPS_MIN[0]);
        }
        Rating::Hard => {
            state.ease = (state.ease - 0.15).max(1.3);
            state.interval_days = (state.interval_days * 1.2).max(1.0);
            state.reps = state.reps.saturating_add(1);
            state.due_at = now_ms + days_ms(state.interval_days);
        }
        Rating::Good => {
            state.interval_days = (state.interval_days * state.ease).max(1.0);
            state.reps = state.reps.saturating_add(1);
            state.due_at = now_ms + days_ms(state.interval_days);
        }
        Rating::Easy => {
            state.ease = (state.ease + 0.15).min(3.0);
            state.interval_days = (state.interval_days * state.ease * 1.3).max(1.0);
            state.reps = state.reps.saturating_add(1);
            state.due_at = now_ms + days_ms(state.interval_days);
        }
    }
}

fn minutes_ms(mins: i64) -> i64 {
    Duration::minutes(mins).num_milliseconds()
}

fn days_ms(days: f64) -> i64 {
    let secs = (days * 86_400.0).round() as i64;
    Duration::seconds(secs.max(60)).num_milliseconds()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_good_graduates() {
        let mut s = LearnerItemState::fresh_unintroduced();
        let now = 1_000_000_i64;
        introduce_into_learning(&mut s, now);
        apply_rating(&mut s, Rating::Good, now);
        assert_eq!(s.phase, ItemPhase::Learning);
        assert_eq!(s.learning_step, 1);
        apply_rating(&mut s, Rating::Good, now + 60_000);
        assert_eq!(s.phase, ItemPhase::Review);
        assert!(s.due_at > now);
        assert!((s.interval_days - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn review_again_enters_relearning() {
        let mut s = LearnerItemState::fresh_unintroduced();
        let now = 1_000_000_i64;
        introduce_into_learning(&mut s, now);
        apply_rating(&mut s, Rating::Good, now);
        apply_rating(&mut s, Rating::Good, now);
        assert_eq!(s.phase, ItemPhase::Review);
        apply_rating(&mut s, Rating::Again, now + 86_400_000);
        assert_eq!(s.phase, ItemPhase::Relearning);
        assert_eq!(s.lapses, 1);
    }

    #[test]
    fn review_good_extends_interval() {
        let mut s = LearnerItemState {
            phase: ItemPhase::Review,
            due_at: 0,
            interval_days: 2.0,
            ease: 2.5,
            learning_step: 0,
            reps: 3,
            lapses: 0,
            introduced: true,
            suspended: false,
            ladder_stage: crate::model::LadderStage::L1,
        };
        let now = 10_000_i64;
        apply_rating(&mut s, Rating::Good, now);
        assert!((s.interval_days - 5.0).abs() < f64::EPSILON);
        assert_eq!(s.phase, ItemPhase::Review);
    }
}
