use crate::opencode::bridge::BridgePromptRequest;

use super::super::super::scene_api::WorldScope;

pub(super) fn world_scope_from_request(request: &BridgePromptRequest) -> WorldScope {
    WorldScope {
        scene_id: request
            .scene_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        entry_id: request
            .entry_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        target_file: request
            .target_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    }
}
