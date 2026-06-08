use anyhow::Result;
use mei_lang_toolchain as toolchain;
use serde_json::json;

use crate::http;
use super::super::args::{
    QueryArgs, QueryCommand, QueryDatasetArgs, QueryMetricArgs, QueryResourceArgs,
};
use super::super::util::{
    ensure_cli_layout_ready, inspect_layout_for_app, parse_cli_filters, print_json_output,
    resolve_cli_source_root, resolve_package_root, scope_json, world_scope_from_selector,
};

pub fn query_command(args: QueryArgs) -> Result<()> {
    match args.command {
        QueryCommand::Dataset(args) => query_dataset_command(args),
        QueryCommand::Metric(args) => query_metric_command(args),
        QueryCommand::Resource(args) => query_resource_command(args),
    }
}

pub fn query_dataset_command(args: QueryDatasetArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let filters = parse_cli_filters(&args.filters)?;
    let columns = if args.columns.is_empty() {
        None
    } else {
        Some(args.columns.as_slice())
    };
    let result = http::scene_api::query_resource_dataset(
        &source_root,
        app_id,
        scope.as_ref(),
        args.id.trim(),
        args.search.as_deref(),
        &filters,
        columns,
        args.limit,
    )?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "query.dataset",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "dataset_id": args.id.trim(),
        "result": result,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

pub fn query_metric_command(args: QueryMetricArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let filters = parse_cli_filters(&args.filters)?;
    let result = http::scene_api::query_resource_dataset_metric(
        &source_root,
        app_id,
        scope.as_ref(),
        args.id.trim(),
        &args.metric_ids,
        args.search.as_deref(),
        &filters,
    )?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "query.metric",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "dataset_id": args.id.trim(),
        "metric_ids": args.metric_ids,
        "result": result,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

pub fn query_resource_command(args: QueryResourceArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let result = toolchain::query_world_asset(&source_root, app_id, scope.as_ref(), args.id.trim())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "query.resource",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "resource_id": args.id.trim(),
        "result": result,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}
