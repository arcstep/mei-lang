use anyhow::Result;
use mei_lang_kernel::Severity;
use mei_lang_toolchain as toolchain;
use serde_json::json;

use super::super::args::CheckArgs;
use super::super::util::{
    compile_options_from_selector, diagnostics_summary, ensure_cli_layout_ready,
    inspect_layout_for_app, print_json_output, resolve_cli_source_root, resolve_package_root,
    watched_files_json,
};
use crate::agent_runtime;

pub fn compile_or_check_command(command: &str, args: CheckArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let source_root = resolve_cli_source_root(&package_root, &args.app.source_root)?;
    let app_id = args.app.app.trim();
    if app_id.is_empty() {
        anyhow::bail!("--app is required");
    }
    let layout = inspect_layout_for_app(&source_root, app_id);
    ensure_cli_layout_ready(&layout)?;
    let options = compile_options_from_selector(&args.app);
    let report = toolchain::compile_report(&source_root, app_id, options.clone())?;
    let compiled = report.compiled;
    let output = json!({
        "schema_version": "mei-cli-v1",
        "command": command,
        "app_id": app_id,
        "source_root": source_root,
        "requested": {
            "scene_id": options.scene,
            "target_file": options.preview_target,
        },
        "active": {
            "scene_id": compiled.active_scene,
            "target_file": compiled.active_target_file,
        },
        "ok": !compiled.diagnostics.iter().any(|item| matches!(item.severity, Severity::Error)),
        "diagnostics_summary": diagnostics_summary(&compiled.diagnostics),
        "diagnostics": compiled.diagnostics,
        "scene_routes": compiled.scene_routes,
        "revision": {
            "token": report.revision_token,
            "components_revision": report.components_revision,
            "watched_files": watched_files_json(&report.watched_files),
        },
        "layout": layout,
    });
    print_json_output(&output, args.app.json)
}
