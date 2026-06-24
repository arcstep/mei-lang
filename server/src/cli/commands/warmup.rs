use anyhow::Result;
use mei_lang_kernel::{discover_board_warmup_suggestions, resolve_app_root, resolve_runtime_warmup_manifest};
use serde_json::json;
use std::collections::BTreeSet;

use super::super::args::{WarmupArgs, WarmupCommand, WarmupSuggestArgs};
use super::super::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};
use crate::agent_runtime;

pub fn warmup_command(args: WarmupArgs) -> Result<()> {
    match args.command {
        WarmupCommand::Suggest(suggest_args) => warmup_suggest_command(suggest_args),
    }
}

pub fn warmup_suggest_command(args: WarmupSuggestArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
    let manifest = resolve_runtime_warmup_manifest(source_root.as_path())?
        .ok_or_else(|| anyhow::anyhow!("warmup manifest unavailable for workspace"))?;
    let app_filter = args
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut suggestions = Vec::new();
    for app in &manifest.apps {
        if app_filter.is_some_and(|filter| filter != app.app_id.trim()) {
            continue;
        }
        let app_root = resolve_app_root(source_root.as_path(), app.app_id.trim());
        let board_suggestions = discover_board_warmup_suggestions(app_root.as_path())?;
        let manual_keys = app
            .datasets
            .iter()
            .map(|entry| {
                format!(
                    "{}|{}",
                    entry.scene_id.as_deref().unwrap_or("").trim(),
                    entry.dataset_id.trim()
                )
            })
            .collect::<BTreeSet<_>>();
        for entry in board_suggestions {
            let key = format!("{}|{}", entry.scene_id, entry.dataset_id);
            if manual_keys.contains(&key) {
                continue;
            }
            suggestions.push(json!({
                "appId": app.app_id,
                "sceneId": entry.scene_id,
                "focus": entry.focus,
                "datasetId": entry.dataset_id,
                "metricId": entry.metric_id,
                "priority": entry.priority,
            }));
        }
    }
    if args.json {
        print_json_output(
            &json!({
            "sourceRoot": source_root.display().to_string(),
            "suggestedCount": suggestions.len(),
            "suggestions": suggestions,
            }),
            true,
        )?;
    } else {
        println!(
            "warmup suggest: {} board-derived entries not in workspace config",
            suggestions.len()
        );
        for item in &suggestions {
            println!(
                "- app={} scene={} dataset={} focus={}",
                item["appId"], item["sceneId"], item["datasetId"], item["focus"]
            );
        }
    }
    Ok(())
}
