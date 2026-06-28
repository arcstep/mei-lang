use std::path::Path;

use mei_lang_kernel::{
    read_links_state, resolve_active_build_identity, resolve_build_footer_label,
};
use serde_json::{json, Value};

pub const BUILD_VERSION: &str = env!("MEI_BUILD_VERSION");
pub const CARGO_PACKAGE_VERSION: &str = env!("MEI_CARGO_PACKAGE_VERSION");
pub const GIT_COMMIT_SHORT: &str = env!("MEI_GIT_COMMIT_SHORT");
pub const GIT_DIRTY: &str = env!("MEI_GIT_DIRTY");
pub const GIT_BRANCH: &str = env!("MEI_GIT_BRANCH");

pub fn binary_descriptor() -> Value {
    json!({
        "crate": "mei-host-shell",
        "build_version": BUILD_VERSION,
        "cargo_package_version": CARGO_PACKAGE_VERSION,
        "git": {
            "commit_short": GIT_COMMIT_SHORT,
            "branch": GIT_BRANCH,
            "dirty": GIT_DIRTY == "true",
        },
    })
}

pub fn workspace_descriptor(workspace_root: &Path) -> Value {
    let identity = resolve_active_build_identity(workspace_root);
    let links = read_links_state(workspace_root).ok();
    let display_label = resolve_build_footer_label(workspace_root);
    json!({
        "toolchain_version": identity.toolchain_version,
        "workspace_version": identity.workspace_version,
        "display_label": display_label,
        "env": {
            "active": links.as_ref().and_then(|state| state.build.active.clone()),
            "candidate": links.as_ref().and_then(|state| state.build.candidate.clone()),
            "previous": links.as_ref().and_then(|state| state.build.previous.clone()),
        },
        "toolchain": {
            "active": links.as_ref().and_then(|state| state.toolchain.active.clone()),
        },
    })
}

pub fn version_descriptor(workspace_root: Option<&Path>, host_started_at_ms: Option<u64>) -> Value {
    let mut value = json!({
        "binary": binary_descriptor(),
    });
    if let Some(workspace_root) = workspace_root {
        let workspace = workspace_descriptor(workspace_root);
        if let Some(display_label) = workspace
            .get("display_label")
            .and_then(Value::as_str)
            .map(str::to_string)
        {
            value["displayLabel"] = Value::String(display_label);
        }
        value["workspace"] = workspace;
    }
    if let Some(host_started_at_ms) = host_started_at_ms {
        value["hostStartedAtMs"] = Value::Number(host_started_at_ms.into());
    }
    value
}

pub fn statusbar_version_label(workspace_root: &Path) -> String {
    format!(
        "{} · shell {}",
        resolve_build_footer_label(workspace_root),
        BUILD_VERSION
    )
}

pub fn statusbar_version_title(workspace_root: &Path) -> String {
    let descriptor = version_descriptor(Some(workspace_root), None);
    serde_json::to_string(&descriptor).unwrap_or_else(|_| BUILD_VERSION.to_string())
}

pub fn fill_host_build_placeholders(mut html: String, workspace_root: &Path) -> String {
    html = html.replace("__MEI_HOST_VERSION__", BUILD_VERSION);
    html = html.replace(
        "__MEI_HOST_VERSION_LABEL__",
        &statusbar_version_label(workspace_root),
    );
    html = html.replace(
        "__MEI_HOST_VERSION_TITLE__",
        &statusbar_version_title(workspace_root),
    );
    html
}

pub fn log_host_identity(workspace_root: Option<&Path>, event: &str) {
    let display_label = workspace_root
        .map(resolve_build_footer_label)
        .unwrap_or_else(|| "workspace=n/a".to_string());
    tracing::info!(
        event = event,
        build_version = %BUILD_VERSION,
        cargo_version = %CARGO_PACKAGE_VERSION,
        git_commit = %GIT_COMMIT_SHORT,
        git_dirty = %(GIT_DIRTY == "true"),
        display_label = %display_label,
        "mei-host-shell"
    );
}

pub fn print_cli_version(workspace_root: Option<&Path>, json_output: bool) -> anyhow::Result<()> {
    let descriptor = version_descriptor(workspace_root, None);
    if json_output {
        println!("{}", serde_json::to_string_pretty(&descriptor)?);
        return Ok(());
    }
    println!("shell.build_version={BUILD_VERSION}");
    println!("shell.cargo_version={CARGO_PACKAGE_VERSION}");
    println!("shell.git_commit={GIT_COMMIT_SHORT}");
    println!("shell.git_branch={GIT_BRANCH}");
    println!("shell.git_dirty={}", GIT_DIRTY == "true");
    if let Some(workspace_root) = workspace_root {
        let workspace = workspace_descriptor(workspace_root);
        if let Some(toolchain) = workspace
            .get("toolchain_version")
            .and_then(Value::as_str)
        {
            println!("workspace.toolchain_version={toolchain}");
        }
        if let Some(ws_ver) = workspace
            .get("workspace_version")
            .and_then(Value::as_str)
        {
            println!("workspace.version={ws_ver}");
        }
        if let Some(env) = workspace.get("env") {
            if let Some(active) = env.get("active").and_then(Value::as_str) {
                println!("env.active={active}");
            }
            if let Some(candidate) = env.get("candidate").and_then(Value::as_str) {
                println!("env.candidate={candidate}");
            }
            if let Some(previous) = env.get("previous").and_then(Value::as_str) {
                println!("env.previous={previous}");
            }
        }
        if let Some(display) = workspace.get("display_label").and_then(Value::as_str) {
            println!("display={display}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::{write_links_state, LinksState};
    use std::fs;
    use tempfile::tempdir;

    fn write_ws(ws: &Path, version: &str) {
        fs::write(
            ws.join("workspace.json"),
            format!(
                r#"{{"schemaVersion":1,"workspace":{{"id":"test","version":"{version}"}}}}"#
            ),
        )
        .expect("write workspace.json");
    }

    #[test]
    fn statusbar_label_includes_workspace_and_shell_build() {
        let tmp = tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
        write_ws(ws, "20260228");
        let mut links = LinksState::default();
        links.toolchain.active = Some("2.0.1".into());
        links.build.active = Some("2.0.1-ws20260228".into());
        write_links_state(ws, &links).expect("write links");
        let label = statusbar_version_label(ws);
        assert!(label.contains("MeiLang 2.0.1"));
        assert!(label.contains("build 2.0.1-ws20260228"));
        assert!(label.contains("shell "));
        assert!(label.contains(BUILD_VERSION));
    }

    #[test]
    fn fill_placeholders_replaces_version_meta_and_statusbar() {
        let tmp = tempdir().expect("tempdir");
        write_ws(tmp.path(), "1");
        let html = fill_host_build_placeholders(
            r#"<meta name="mei-host-version" content="__MEI_HOST_VERSION__"/>
            <span id="mei-status-host-version" title="__MEI_HOST_VERSION_TITLE__">__MEI_HOST_VERSION_LABEL__</span>"#
                .to_string(),
            tmp.path(),
        );
        assert!(!html.contains("__MEI_HOST_VERSION__"));
        assert!(!html.contains("__MEI_HOST_VERSION_LABEL__"));
        assert!(html.contains(BUILD_VERSION));
        assert!(html.contains("shell "));
    }
}
