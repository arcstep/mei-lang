use anyhow::Result;
use mei_lang_kernel::{host_runtime_capabilities_catalog, host_runtime_contract_descriptor};
use mei_lang_toolchain as toolchain;
use serde_json::json;

use super::super::args::{RuntimeArgs, RuntimeCommand, RuntimePeekArgs};
use super::super::util::{
    ensure_cli_layout_ready, inspect_layout_for_app, print_json_output, resolve_cli_source_root,
    resolve_package_root, scope_json, world_scope_from_selector,
};

pub fn runtime_command(args: RuntimeArgs) -> Result<()> {
    match args.command {
        RuntimeCommand::Peek(args) => runtime_peek_command(args),
    }
}

fn runtime_peek_command(args: RuntimePeekArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let scope = world_scope_from_selector(&args.app);
    let result =
        toolchain::query_world_runtime(&source_root, app_id, scope.as_ref(), args.trace_limit)?;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": "runtime.peek",
        "app_id": app_id,
        "scope": scope_json(scope.as_ref()),
        "runtime_capabilities": host_runtime_capabilities_catalog(),
        "host_contract": host_runtime_contract_descriptor(),
        "result": result,
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}
