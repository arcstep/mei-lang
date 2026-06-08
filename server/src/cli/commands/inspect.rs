use anyhow::Result;
use mei_lang_toolchain as toolchain;
use serde_json::json;

use super::super::args::{
    InspectArgs, InspectCommand, InspectInventoryArgs, InspectLayoutArgs, InspectWorldArgs,
};
use super::super::util::{
    ensure_cli_layout_ready, inspect_layout_for_app, print_json_output, resolve_cli_source_root,
    resolve_package_root, scope_json, world_scope_from_selector,
};

pub fn inspect_command(args: InspectArgs) -> Result<()> {
    match args.command {
        InspectCommand::World(args) => inspect_world_command(args),
        InspectCommand::Inventory(args) => inspect_inventory_command(args),
        InspectCommand::Layout(args) => inspect_layout_command(args),
    }
}

pub fn inspect_world_command(args: InspectWorldArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let snapshot = toolchain::build_world_context_snapshot(&source_root, app_id, scope.as_ref())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "inspect.world",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "world_context": snapshot,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

pub fn inspect_inventory_command(args: InspectInventoryArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let snapshot = toolchain::build_world_context_snapshot(&source_root, app_id, scope.as_ref())?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "inspect.inventory",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "active_target_file": snapshot.active_target_file,
        "inventory": snapshot.resource_inventory,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}

pub fn inspect_layout_command(args: InspectLayoutArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "inspect.layout",
        "app_id": app_id,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}
