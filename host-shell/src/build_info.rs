use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use mei_lang_kernel::{
    discover_apps, read_links_state, resolve_active_build_identity_with_hint,
    resolve_build_footer_label_with_hint, resolve_version_display_identity_with_hint,
    resolve_workspace_app_build_generations, VersionDisplayIdentity,
};
use serde_json::{json, Value};

pub const BUILD_VERSION: &str = env!("MEI_BUILD_VERSION");
pub const CARGO_PACKAGE_VERSION: &str = env!("MEI_CARGO_PACKAGE_VERSION");
pub const GIT_COMMIT_SHORT: &str = env!("MEI_GIT_COMMIT_SHORT");
pub const GIT_DIRTY: &str = env!("MEI_GIT_DIRTY");
pub const GIT_BRANCH: &str = env!("MEI_GIT_BRANCH");

pub fn meilang_version_hint() -> &'static str {
    BUILD_VERSION
}

pub fn binary_descriptor() -> Value {
    json!({
        "crate": "mei-host-shell",
        "build_version": BUILD_VERSION,
        "cargo_package_version": CARGO_PACKAGE_VERSION,
        "meilangVersion": BUILD_VERSION,
        "git": {
            "commit_short": GIT_COMMIT_SHORT,
            "branch": GIT_BRANCH,
            "dirty": GIT_DIRTY == "true",
        },
    })
}

fn version_display(workspace_root: &Path) -> VersionDisplayIdentity {
    resolve_version_display_identity_with_hint(workspace_root, Some(meilang_version_hint()))
        .unwrap_or_else(|err| panic!("{err}"))
}

pub fn workspace_descriptor(workspace_root: &Path) -> Value {
    let identity =
        resolve_active_build_identity_with_hint(workspace_root, Some(meilang_version_hint()))
            .unwrap_or_else(|err| panic!("{err}"));
    let display = version_display(workspace_root);
    let links = read_links_state(workspace_root).ok();
    let app_ids: Vec<String> = discover_apps(workspace_root)
        .unwrap_or_default()
        .into_iter()
        .map(|app| app.id)
        .collect();
    let current_by_app =
        resolve_workspace_app_build_generations(workspace_root, &app_ids).unwrap_or_default();
    let display_label =
        resolve_build_footer_label_with_hint(workspace_root, Some(meilang_version_hint()));
    json!({
        "meilangVersion": display.meilang_version,
        "buildGeneration": display.build_generation,
        "buildDisplayTag": display.build_display_tag,
        "toolchain_version": identity.meilang_version,
        "workspace_version": identity.workspace_version,
        "display_label": display_label,
        "env": {
            "currentByApp": current_by_app,
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
        if let Some(meilang) = workspace.get("meilangVersion").cloned() {
            value["meilangVersion"] = meilang;
        }
        if let Some(build_generation) = workspace.get("buildGeneration").cloned() {
            value["buildGeneration"] = build_generation;
        }
        if let Some(build_display_tag) = workspace.get("buildDisplayTag").cloned() {
            value["buildDisplayTag"] = build_display_tag;
        }
        value["workspace"] = workspace;
    }
    if let Some(host_started_at_ms) = host_started_at_ms {
        value["hostStartedAtMs"] = Value::Number(host_started_at_ms.into());
    }
    value
}

pub fn statusbar_version_label(workspace_root: &Path) -> String {
    mei_lang_kernel::resolve_workspace_footer_label_with_hint(
        workspace_root,
        Some(meilang_version_hint()),
    )
}

/// Terminal banner line: `mei-host-shell {build} · MeiLang x.y.z · Build WS-…`
pub fn host_version_banner_line(workspace_root: &Path) -> String {
    let footer = mei_lang_kernel::resolve_build_footer_label_with_hint(
        workspace_root,
        Some(meilang_version_hint()),
    );
    format!("mei-host-shell {BUILD_VERSION} · {footer}")
}

pub fn statusbar_version_title(workspace_root: &Path) -> String {
    let descriptor = version_descriptor(Some(workspace_root), None);
    serde_json::to_string(&descriptor).unwrap_or_else(|_| BUILD_VERSION.to_string())
}

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

fn host_asset_version() -> String {
    let dist_root = package_root().join("app/assets/dist");
    let newest_stamp = [
        dist_root.join("access.bundle.js"),
        dist_root.join("manage.bundle.js"),
        dist_root.join("styles.bundle.css"),
        dist_root.join("shoelace.bundle.js"),
    ]
    .into_iter()
    .filter_map(|path| {
        let modified = std::fs::metadata(path).ok()?.modified().ok()?;
        let elapsed = modified.duration_since(UNIX_EPOCH).ok()?;
        Some(elapsed.as_millis())
    })
    .max();
    match newest_stamp {
        Some(stamp) => format!("{BUILD_VERSION}.{stamp}"),
        None => BUILD_VERSION.to_string(),
    }
}

pub fn fill_host_build_placeholders(mut html: String, workspace_root: &Path) -> String {
    html = html.replace("__MEI_HOST_VERSION__", BUILD_VERSION);
    html = html.replace("__MEI_HOST_ASSET_VERSION__", host_asset_version().as_str());
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

pub fn fill_host_compliance_placeholders(mut html: String, workspace_root: &Path) -> String {
    let workspace = mei_lang_kernel::load_workspace_config(workspace_root);
    html = html.replace(
        "__MEI_HOST_ICP_RECORD__",
        workspace.compliance.icp_record_trimmed().unwrap_or(""),
    );
    html = html.replace(
        "__MEI_HOST_PSB_RECORD__",
        workspace.compliance.psb_record_trimmed().unwrap_or(""),
    );
    html = html.replace(
        "__MEI_HOST_COPYRIGHT__",
        workspace.compliance.copyright_trimmed().unwrap_or(""),
    );
    html = html.replace(
        "__MEI_WORKSPACE_LABEL__",
        workspace
            .workspace
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(""),
    );
    html
}

pub fn fill_page_shell_placeholders(html: String, workspace_root: &Path) -> String {
    fill_host_compliance_placeholders(
        fill_host_build_placeholders(html, workspace_root),
        workspace_root,
    )
}

pub fn log_host_identity(workspace_root: Option<&Path>, event: &str) {
    let display_label = workspace_root
        .map(|root| resolve_build_footer_label_with_hint(root, Some(meilang_version_hint())))
        .unwrap_or_else(|| "workspace=n/a".to_string());
    tracing::info!(
        event = event,
        build_version = %BUILD_VERSION,
        cargo_version = %CARGO_PACKAGE_VERSION,
        meilang_version = %CARGO_PACKAGE_VERSION,
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
    println!("shell.meilang_version={CARGO_PACKAGE_VERSION}");
    println!("shell.git_commit={GIT_COMMIT_SHORT}");
    println!("shell.git_branch={GIT_BRANCH}");
    println!("shell.git_dirty={}", GIT_DIRTY == "true");
    if let Some(workspace_root) = workspace_root {
        let workspace = workspace_descriptor(workspace_root);
        if let Some(meilang) = workspace.get("meilangVersion").and_then(Value::as_str) {
            println!("workspace.meilang_version={meilang}");
        }
        if let Some(build_generation) = workspace.get("buildGeneration").and_then(Value::as_str) {
            println!("workspace.build_generation={build_generation}");
        }
        if let Some(ws_ver) = workspace.get("workspace_version").and_then(Value::as_str) {
            println!("workspace.version={ws_ver}");
        }
        if let Some(env) = workspace.get("env") {
            if let Some(current_by_app) = env.get("currentByApp").and_then(Value::as_object) {
                for (app_id, current) in current_by_app {
                    if let Some(current) = current.as_str() {
                        println!("app={app_id} current={current}");
                    }
                }
            }
            if let Some(candidate) = env.get("candidate").and_then(Value::as_str) {
                println!("links.candidate={candidate}");
            }
            if let Some(previous) = env.get("previous").and_then(Value::as_str) {
                println!("links.previous={previous}");
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
                r#"{{"schemaVersion":2,"workspace":{{"id":"test","version":"{version}"}},"build":{{"generation":{{"dateSource":"manual","date":"{version}","fixver":0}}}}}}"#
            ),
        )
        .expect("write workspace.json");
    }

    fn setup_demo_app(ws: &Path) {
        fs::create_dir_all(ws.join("apps/demo/src")).expect("mkdir app");
        fs::write(
            ws.join("apps/demo/app.config.json"),
            r#"{"schemaVersion":1,"entry":{"main":"main.mei"}}"#,
        )
        .expect("write app config");
    }

    #[test]
    fn statusbar_label_shows_meilang_and_build_generation() {
        let tmp = tempdir().expect("tempdir");
        let ws = tmp.path();
        fs::create_dir_all(ws.join("deploy/state")).expect("mkdir");
        fs::write(
            ws.join("workspace.json"),
            r#"{"schemaVersion":2,"workspace":{"defaultApp":"demo"},"build":{"generation":{"dateSource":"auto","date":"20260228","fixver":0}}}"#,
        )
        .expect("write workspace.json");
        setup_demo_app(ws);
        mei_lang_kernel::attach_build_generation(ws, &[String::from("demo")], "WS-20260228.0")
            .expect("attach");
        let mut links = LinksState::default();
        links.toolchain.active = Some("2.0.1".into());
        write_links_state(ws, &links).expect("write links");
        let expected = mei_lang_kernel::resolve_build_generation_for_prebuild(ws).tag;
        let label = statusbar_version_label(ws);
        assert!(label.contains("MeiLang"));
        assert!(label.contains(BUILD_VERSION));
        assert!(label.contains(&format!("Build {expected}")));
        assert!(!label.contains("Build WS-20260228.0"));
        let api_label = resolve_build_footer_label_with_hint(ws, Some("2.0.8"));
        assert!(api_label.contains("MeiLang 2.0.8"));
        assert!(api_label.contains(&format!("Build {expected}")));
        assert!(!api_label.contains("build WS-20260228.0 · build"));
    }

    #[test]
    fn fill_placeholders_replaces_version_meta_and_statusbar() {
        let tmp = tempdir().expect("tempdir");
        write_ws(tmp.path(), "20260228");
        let html = fill_page_shell_placeholders(
            r#"<meta name="mei-host-version" content="__MEI_HOST_VERSION__"/>
            <span id="mei-status-host-version" title="__MEI_HOST_VERSION_TITLE__">__MEI_HOST_VERSION_LABEL__</span>"#
                .to_string(),
            tmp.path(),
        );
        assert!(!html.contains("__MEI_HOST_VERSION__"));
        assert!(!html.contains("__MEI_HOST_VERSION_LABEL__"));
        assert!(html.contains(BUILD_VERSION));
        assert!(!html.contains("shell "));
        assert!(html.contains("MeiLang"));
    }

    #[test]
    fn fill_compliance_placeholders_replaces_icp_psb_copyright_and_workspace_label() {
        let tmp = tempdir().expect("tempdir");
        fs::write(
            tmp.path().join("workspace.json"),
            r#"{
                "schemaVersion": 1,
                "workspace": { "id": "test", "label": "Demo Workspace" },
                "compliance": {
                    "icpRecord": "京ICP备00000000号",
                    "psbRecord": "京公网安备110000000000号",
                    "copyright": "© 2026 Example"
                }
            }"#,
        )
        .expect("write workspace.json");
        let html = fill_page_shell_placeholders(
            r#"<meta name="mei-host-icp-record" content="__MEI_HOST_ICP_RECORD__"/>
            <meta name="mei-host-psb-record" content="__MEI_HOST_PSB_RECORD__"/>
            <meta name="mei-host-copyright" content="__MEI_HOST_COPYRIGHT__"/>
            <meta name="mei-workspace-label" content="__MEI_WORKSPACE_LABEL__"/>"#
                .to_string(),
            tmp.path(),
        );
        assert!(!html.contains("__MEI_HOST_ICP_RECORD__"));
        assert!(!html.contains("__MEI_HOST_PSB_RECORD__"));
        assert!(!html.contains("__MEI_HOST_COPYRIGHT__"));
        assert!(!html.contains("__MEI_WORKSPACE_LABEL__"));
        assert!(html.contains("京ICP备00000000号"));
        assert!(html.contains("Demo Workspace"));
    }
}
