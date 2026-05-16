use std::collections::BTreeMap;

use crate::SceneContract;

use super::super::{
    RuntimeCellView, RuntimeEntityView, RuntimeSceneView, RuntimeState,
};

fn entity_flags(state: &RuntimeState, entity_id: &str) -> BTreeMap<String, bool> {
    let prefix = format!("{entity_id}.");
    state
        .flags
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix(&prefix)
                .map(|short| (short.to_string(), *value))
        })
        .collect()
}

fn cell_status_key(cell_id: &str) -> String {
    format!("cell:{cell_id}")
}

fn entity_interaction_target(contract: &SceneContract, entity_id: &str) -> Option<String> {
    contract
        .flow
        .as_ref()
        .and_then(|flow| {
            flow.interactions
                .iter()
                .find(|rule| rule.target == entity_id)
        })
        .map(|rule| rule.target.clone())
}

fn entity_in_inventory(state: &RuntimeState, entity_id: &str) -> bool {
    state.inventory.iter().any(|owned| owned == entity_id)
}

fn interaction_is_available(
    require: Option<&crate::RuleRequireDecl>,
    state: &RuntimeState,
) -> bool {
    let Some(require) = require else {
        return true;
    };
    if require.require_type == "has" {
        return state.inventory.iter().any(|owned| owned == &require.value);
    }
    true
}

fn cell_hazard_state(
    state: &RuntimeState,
    cell_id: &str,
    declared_hazard: Option<String>,
) -> Option<String> {
    state
        .statuses
        .get(&cell_status_key(cell_id))
        .cloned()
        .or(declared_hazard)
}

fn cell_timer_remaining(state: &RuntimeState, cell_id: &str) -> Option<f64> {
    let subject_ref = cell_status_key(cell_id);
    state
        .subject_timers
        .iter()
        .filter(|timer| timer.subject_ref == subject_ref)
        .map(|timer| (timer.due_at - state.clock.current_time).max(0.0))
        .min_by(|left, right| left.total_cmp(right))
}

fn cell_interaction_target(contract: &SceneContract, cell_id: &str) -> Option<String> {
    let Some(flow) = &contract.flow else {
        return None;
    };
    let prefixed = cell_status_key(cell_id);
    flow.interactions
        .iter()
        .find(|rule| rule.target == prefixed || rule.target == cell_id)
        .map(|rule| rule.target.clone())
}

fn cell_interaction_available(
    contract: &SceneContract,
    state: &RuntimeState,
    cell_id: &str,
) -> bool {
    let Some(flow) = &contract.flow else {
        return false;
    };
    let prefixed = cell_status_key(cell_id);
    flow.interactions
        .iter()
        .find(|rule| rule.target == prefixed || rule.target == cell_id)
        .map(|rule| interaction_is_available(rule.require.as_ref(), state))
        .unwrap_or(false)
}

fn cell_is_key_target(contract: &SceneContract, cell_id: &str) -> bool {
    let Some(flow) = &contract.flow else {
        return false;
    };
    let prefixed = cell_status_key(cell_id);
    flow.interactions.iter().any(|rule| {
        (rule.target == prefixed || rule.target == cell_id)
            && rule
                .require
                .as_ref()
                .map(|require| require.require_type == "has")
                .unwrap_or(false)
    })
}

fn build_cells(
    contract: &SceneContract,
    state: &RuntimeState,
    entities: &[RuntimeEntityView],
) -> Vec<RuntimeCellView> {
    let mut cells = Vec::new();
    if let Some(world) = &contract.world {
        if let Some(topology) = &world.topology {
            for row in 1..=topology.rows {
                for col in 1..=topology.cols {
                    let id = format!("r{row}c{col}");
                    let declared = topology.cells.iter().find(|cell| cell.id == id);
                    let occupants = entities
                        .iter()
                        .filter(|entity| {
                            entity.slot.as_deref() == Some(id.as_str()) && !entity.in_inventory
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    let declared_hazard = declared.and_then(|cell| cell.hazard_state.clone());
                    let hazard_state = cell_hazard_state(state, &id, declared_hazard);
                    let timer_remaining = cell_timer_remaining(state, &id);
                    let interaction_target = cell_interaction_target(contract, &id);
                    let interaction_available = cell_interaction_available(contract, state, &id);
                    let key_target = cell_is_key_target(contract, &id);
                    cells.push(RuntimeCellView {
                        id,
                        surface_kind: declared.and_then(|cell| cell.surface_kind.clone()),
                        flammable: declared.and_then(|cell| cell.flammable),
                        walkable: declared.and_then(|cell| cell.walkable),
                        occupiable: declared.and_then(|cell| cell.occupiable),
                        hazard_state,
                        hazard_timer_remaining: timer_remaining,
                        hazard_timer_seconds: timer_remaining.map(|value| value.ceil() as i64),
                        clickable: interaction_target.is_some() && interaction_available,
                        interaction_target,
                        key_target,
                        tags: declared.map(|cell| cell.tags.clone()).unwrap_or_default(),
                        entities: occupants,
                    });
                }
            }
        }
    }
    cells
}

pub fn project_runtime_view(contract: &SceneContract, state: &RuntimeState) -> RuntimeSceneView {
    let entities = contract
        .world
        .as_ref()
        .map(|world| {
            world
                .entities
                .iter()
                .map(|entity| {
                    let in_inventory = entity_in_inventory(state, &entity.id);
                    let interaction_target = entity_interaction_target(contract, &entity.id);
                    let interaction_available = contract
                        .flow
                        .as_ref()
                        .and_then(|flow| {
                            flow.interactions
                                .iter()
                                .find(|rule| rule.target == entity.id)
                        })
                        .map(|rule| interaction_is_available(rule.require.as_ref(), state))
                        .unwrap_or(false);
                    RuntimeEntityView {
                        id: entity.id.clone(),
                        kind: entity.kind.clone(),
                        label: entity.label.clone(),
                        slot: (!in_inventory)
                            .then(|| state.placements.get(&entity.id).cloned())
                            .flatten(),
                        status: state.statuses.get(&entity.id).cloned(),
                        interaction_target: interaction_target.clone(),
                        clickable: interaction_target.is_some() && interaction_available,
                        in_inventory,
                        flags: entity_flags(state, &entity.id),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let available_actions = match state.phase.as_str() {
        "ready" => vec!["start".to_string()],
        "running" => {
            let mut actions = vec![
                "tick".to_string(),
                "rate_half".to_string(),
                "rate_normal".to_string(),
                "rate_double".to_string(),
            ];
            if state.clock.paused {
                actions.push("resume".to_string());
            } else {
                actions.push("pause".to_string());
            }
            actions
        }
        _ => vec!["restart".to_string()],
    };
    RuntimeSceneView {
        scene_id: contract.scene.id.clone(),
        goal: contract.scene.goal.clone(),
        profile: contract.scene.profile.clone(),
        summary: contract.scene.summary.clone(),
        phase: state.phase.clone(),
        result: state.result.clone(),
        reason: state.reason.clone(),
        outcome_state: state.phase.clone(),
        outcome_message: state.reason.clone(),
        countdown: state.countdown,
        current_time: state.clock.current_time,
        time_unit: state.clock.time_unit.clone(),
        clock_paused: state.clock.paused,
        time_rate: state.clock.rate,
        inventory: state.inventory.clone(),
        cells: build_cells(contract, state, &entities),
        subject_timers: state.subject_timers.clone(),
        entities,
        available_actions,
        start_label: contract
            .flow
            .as_ref()
            .and_then(|flow| flow.start.as_ref())
            .and_then(|start| start.action_label.clone()),
    }
}
