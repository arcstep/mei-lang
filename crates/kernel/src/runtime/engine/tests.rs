use crate::runtime::types::RuntimeIntent;

use super::snapshot::{test_cell_timer_contract, test_contract};
use super::{project_runtime_view, runtime_step};

#[test]
fn runtime_flow_reaches_success() {
    let contract = test_contract();
    let running = runtime_step(
        &contract,
        None,
        &RuntimeIntent {
            kind: "start".to_string(),
            target: None,
        },
    );
    assert_eq!(running.phase, "running");
    let with_item = runtime_step(
        &contract,
        Some(running),
        &RuntimeIntent {
            kind: "click".to_string(),
            target: Some("wall_extinguisher".to_string()),
        },
    );
    let done = runtime_step(
        &contract,
        Some(with_item),
        &RuntimeIntent {
            kind: "click".to_string(),
            target: Some("room_fire".to_string()),
        },
    );
    assert_eq!(done.phase, "success");
    assert_eq!(
        done.statuses.get("room_fire").map(String::as_str),
        Some("out")
    );
    let view = project_runtime_view(&contract, &done);
    assert_eq!(view.phase, "success");
}

#[test]
fn runtime_flow_times_out() {
    let contract = test_contract();
    let mut state = runtime_step(
        &contract,
        None,
        &RuntimeIntent {
            kind: "start".to_string(),
            target: None,
        },
    );
    for _ in 0..3 {
        state = runtime_step(
            &contract,
            Some(state),
            &RuntimeIntent {
                kind: "tick".to_string(),
                target: None,
            },
        );
    }
    assert_eq!(state.phase, "fail");
    assert_eq!(state.reason.as_deref(), Some("timeout_or_player_dead"));
}

#[test]
fn runtime_subject_timer_drives_cell_hazard_and_click_success() {
    let contract = test_cell_timer_contract();
    let started = runtime_step(
        &contract,
        None,
        &RuntimeIntent {
            kind: "start".to_string(),
            target: None,
        },
    );
    assert_eq!(started.subject_timers.len(), 1);
    assert_eq!(started.subject_timers[0].subject_ref, "cell:r1c1");

    let after_tick = runtime_step(
        &contract,
        Some(started),
        &RuntimeIntent {
            kind: "tick".to_string(),
            target: None,
        },
    );
    assert_eq!(
        after_tick.statuses.get("cell:r1c1").map(String::as_str),
        Some("burning")
    );
    assert!(after_tick.subject_timers.is_empty());

    let with_item = runtime_step(
        &contract,
        Some(after_tick),
        &RuntimeIntent {
            kind: "click".to_string(),
            target: Some("extinguisher_1".to_string()),
        },
    );
    let pickup_view = project_runtime_view(&contract, &with_item);
    let extinguisher = pickup_view
        .entities
        .iter()
        .find(|entity| entity.id == "extinguisher_1")
        .expect("extinguisher should exist");
    assert!(extinguisher.in_inventory);
    assert_eq!(extinguisher.slot, None);
    let tool_cell = pickup_view
        .cells
        .iter()
        .find(|cell| cell.id == "r2c2")
        .expect("r2c2 should exist");
    assert!(tool_cell.entities.is_empty());

    let success = runtime_step(
        &contract,
        Some(with_item),
        &RuntimeIntent {
            kind: "click".to_string(),
            target: Some("cell:r1c1".to_string()),
        },
    );
    assert_eq!(success.phase, "success");
    assert_eq!(success.reason.as_deref(), Some("cell_fire_out"));

    let view = project_runtime_view(&contract, &success);
    let cell = view
        .cells
        .iter()
        .find(|cell| cell.id == "r1c1")
        .expect("r1c1 should exist");
    assert_eq!(cell.hazard_state.as_deref(), Some("out"));
    assert_eq!(cell.interaction_target.as_deref(), Some("cell:r1c1"));
    assert!(cell.clickable);
}

#[test]
fn runtime_clock_supports_pause_resume_and_rate() {
    let contract = test_contract();
    let mut state = runtime_step(
        &contract,
        None,
        &RuntimeIntent {
            kind: "start".to_string(),
            target: None,
        },
    );
    assert_eq!(state.countdown, 3);
    assert_eq!(state.clock.current_time, 0.0);
    assert_eq!(state.clock.rate, 1.0);
    assert!(!state.clock.paused);

    state = runtime_step(
        &contract,
        Some(state),
        &RuntimeIntent {
            kind: "rate_half".to_string(),
            target: None,
        },
    );
    assert_eq!(state.clock.rate, 0.5);
    state = runtime_step(
        &contract,
        Some(state),
        &RuntimeIntent {
            kind: "tick".to_string(),
            target: None,
        },
    );
    assert_eq!(state.countdown, 3);
    state = runtime_step(
        &contract,
        Some(state),
        &RuntimeIntent {
            kind: "tick".to_string(),
            target: None,
        },
    );
    assert_eq!(state.countdown, 2);
    assert_eq!(state.clock.current_time, 1.0);

    state = runtime_step(
        &contract,
        Some(state),
        &RuntimeIntent {
            kind: "pause".to_string(),
            target: None,
        },
    );
    let paused_countdown = state.countdown;
    let paused_time = state.clock.current_time;
    state = runtime_step(
        &contract,
        Some(state),
        &RuntimeIntent {
            kind: "tick".to_string(),
            target: None,
        },
    );
    assert_eq!(state.countdown, paused_countdown);
    assert_eq!(state.clock.current_time, paused_time);

    state = runtime_step(
        &contract,
        Some(state),
        &RuntimeIntent {
            kind: "resume".to_string(),
            target: None,
        },
    );
    state = runtime_step(
        &contract,
        Some(state),
        &RuntimeIntent {
            kind: "rate_double".to_string(),
            target: None,
        },
    );
    state = runtime_step(
        &contract,
        Some(state),
        &RuntimeIntent {
            kind: "tick".to_string(),
            target: None,
        },
    );
    assert_eq!(state.phase, "fail");
    assert_eq!(state.reason.as_deref(), Some("timeout_or_player_dead"));
}

#[test]
fn runtime_view_exposes_clock_actions() {
    let contract = test_contract();
    let running = runtime_step(
        &contract,
        None,
        &RuntimeIntent {
            kind: "start".to_string(),
            target: None,
        },
    );
    let running_view = project_runtime_view(&contract, &running);
    assert!(running_view
        .available_actions
        .iter()
        .any(|action| action == "pause"));
    assert!(running_view
        .available_actions
        .iter()
        .any(|action| action == "rate_half"));
    assert_eq!(running_view.time_rate, 1.0);
    assert!(!running_view.clock_paused);

    let paused = runtime_step(
        &contract,
        Some(running),
        &RuntimeIntent {
            kind: "pause".to_string(),
            target: None,
        },
    );
    let paused_view = project_runtime_view(&contract, &paused);
    assert!(paused_view
        .available_actions
        .iter()
        .any(|action| action == "resume"));
    assert!(!paused_view
        .available_actions
        .iter()
        .any(|action| action == "pause"));
}
