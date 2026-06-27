//! Copy-paste CLI hints for block/layer fast-loop diagnostics.

use crate::prebuild::PrebuildWarningReport;

pub fn block_eval_hint(
    workspace_flag: &str,
    app_id: &str,
    scene_id: Option<&str>,
    target_file: Option<&str>,
    owner: &str,
    metrics: &[String],
) -> String {
    let scope = scene_id
        .map(|value| format!(" --scope {value}"))
        .unwrap_or_default();
    let target = target_file
        .map(|value| format!(" --target {value}"))
        .unwrap_or_default();
    let metrics_flag = if metrics.is_empty() {
        String::new()
    } else {
        format!(
            " {}",
            metrics
                .iter()
                .map(|metric| format!("--metrics {metric}"))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    format!(
        "mei-toolchain block eval {workspace_flag} --app {app_id}{scope}{target} --owner {owner}{metrics_flag} --verbose"
    )
}

pub fn layer_verify_hint(workspace_flag: &str, app_id: &str, layer: &str) -> String {
    format!("mei-toolchain layer verify {workspace_flag} --app {app_id} --layer {layer}")
}

pub fn block_list_hint(workspace_flag: &str, app_id: &str, states: &str) -> String {
    format!("mei-toolchain block list {workspace_flag} --app {app_id} --state {states}")
}

pub fn block_compile_hint(workspace_flag: &str, app_id: &str, target: &str) -> String {
    format!(
        "mei-toolchain block compile {workspace_flag} --app {app_id} --node scene_payload:{target}"
    )
}

pub fn prebuild_warning_hint(
    workspace_flag: &str,
    app_id: &str,
    warning: &PrebuildWarningReport,
) -> Option<String> {
    let category = warning.category.as_str();
    if category.contains("metric_response") || category.contains("metric_eval") {
        let owner = warning
            .dataset_selector
            .as_deref()
            .unwrap_or("__world_metrics__");
        return Some(block_eval_hint(
            workspace_flag,
            app_id,
            warning.scene_id.as_deref(),
            warning.target_file.as_deref(),
            owner,
            warning
                .metric_id
                .as_ref()
                .map(|metric| vec![metric.clone()])
                .unwrap_or_default()
                .as_slice(),
        ));
    }
    if category.contains("mcg") || category.contains("cas") || category.contains("bundle") {
        return Some(layer_verify_hint(workspace_flag, app_id, "mcg"));
    }
    if category.contains("mrg") || category.contains("slot") {
        return Some(layer_verify_hint(workspace_flag, app_id, "mrg"));
    }
    None
}

pub fn fast_loop_hints(workspace_flag: &str, app_id: &str) -> [String; 3] {
    [
        format!(
            "mei-toolchain block eval {workspace_flag} --app {app_id} --scope home --target src/scenes/home.mei --owner <owner> --verbose"
        ),
        layer_verify_hint(workspace_flag, app_id, "mcg"),
        block_list_hint(workspace_flag, app_id, "failed"),
    ]
}

pub fn collect_prebuild_failed_block_hints(
    source_root: &std::path::Path,
    report: &crate::prebuild::PrebuildReport,
) -> Vec<String> {
    let workspace_flag = format!("--workspace {}", source_root.display());
    let mut hints = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for app in &report.apps {
        for warning in &app.warnings {
            if let Some(hint) = prebuild_warning_hint(workspace_flag.as_str(), app.app_id.as_str(), warning)
            {
                if seen.insert(hint.clone()) {
                    hints.push(hint);
                }
            }
        }
    }
    hints
}
