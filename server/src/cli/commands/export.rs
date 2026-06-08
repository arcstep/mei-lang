use anyhow::Result;
use mei_lang_toolchain::{self as toolchain, HeadlessExportOptions};
use super::super::args::{
    ExportArgs, ExportCommand, ExportContractsArgs, ExportEvalPlanArgs, ExportInventoryArgs,
    ExportRuntimeTraceArgs, ExportSemanticDagArgs,
};
use super::super::util::{
    attach_layout_to_envelope, ensure_cli_layout_ready, inspect_layout_for_app, parse_cli_filters,
    print_json_output, resolve_cli_source_root, resolve_package_root, world_scope_from_selector,
};

pub fn export_command(args: ExportArgs) -> Result<()> {
    match args.command {
        ExportCommand::Inventory(args) => export_inventory_command(args),
        ExportCommand::SemanticDag(args) => export_semantic_dag_command(args),
        ExportCommand::Contracts(args) => export_contracts_command(args),
        ExportCommand::EvalPlan(args) => export_eval_plan_command(args),
        ExportCommand::RuntimeTrace(args) => export_runtime_trace_command(args),
    }
}

pub fn export_inventory_command(args: ExportInventoryArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let mut envelope = toolchain::export_inventory_snapshot(
        &source_root,
        app_id,
        &scope,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

pub fn export_semantic_dag_command(args: ExportSemanticDagArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let mut envelope = toolchain::export_semantic_dag(
        &source_root,
        app_id,
        &scope,
        args.dataset_id.trim(),
        &args.metric_ids,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

pub fn export_contracts_command(args: ExportContractsArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let mut envelope = toolchain::export_analysis_contracts(
        &source_root,
        app_id,
        &scope,
        args.dataset_id.trim(),
        &args.metric_ids,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

pub fn export_eval_plan_command(args: ExportEvalPlanArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let filters = parse_cli_filters(&args.filters)?;
    let mut envelope = toolchain::export_eval_plan(
        &source_root,
        app_id,
        &scope,
        args.dataset_id.trim(),
        &args.metric_ids,
        args.search.as_deref(),
        &filters,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}

pub fn export_runtime_trace_command(args: ExportRuntimeTraceArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app).unwrap_or_default();
    let mut envelope = toolchain::export_runtime_trace(
        &source_root,
        app_id,
        &scope,
        args.trace_limit,
        HeadlessExportOptions {
            write_store: args.write_store,
        },
    )?;
    attach_layout_to_envelope(&mut envelope, &layout)?;
    print_json_output(&envelope, args.app.json)
}
