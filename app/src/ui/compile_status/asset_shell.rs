fn is_mei_script_target(target: &str) -> bool {
    target.trim().ends_with(".mei")
}

pub(crate) fn is_static_workspace_asset_target(target: &str) -> bool {
    let trimmed = target.trim();
    !trimmed.is_empty() && !is_mei_script_target(trimmed)
}
