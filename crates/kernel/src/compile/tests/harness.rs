use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn temp_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("mei-lang-kernel-{name}-{nonce}"))
}

pub(super) fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(path, content).expect("write file");
}

/// Optional private workspace (`MEI_TEST_WORKSPACE` only; never sibling join).
pub(super) fn optional_external_workspace() -> Option<PathBuf> {
    let raw = std::env::var("MEI_TEST_WORKSPACE").ok()?;
    let path = PathBuf::from(raw.trim());
    if path.as_os_str().is_empty() || !path.is_dir() {
        return None;
    }
    Some(path.canonicalize().unwrap_or(path))
}

/// Package / monorepo root used only for in-repo temp fixtures (not sibling workspaces).
pub(super) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("mei-lang package root")
}

/// 开发 profile：由 `MEI_TEST_WORKSPACE` 注入（通常指向本地 `ws-dev`）。
pub(super) fn dev_workspace_root() -> Option<PathBuf> {
    optional_external_workspace()
}

pub(super) fn dev_examples_root() -> Option<PathBuf> {
    Some(dev_workspace_root()?.join("examples"))
}

pub(super) fn build_regression_workspace_root() -> PathBuf {
    let root = temp_root("regression-workspace");
    let suite = root.join("regression-suite");
    for app_id in [
        "ds-01-dataset-baseline",
        "cockpit-01-composition-shell",
        "cockpit-02-multi-entry",
        "sim-01-fire-baseline",
        "chart-01-echarts",
        "sim-02-fire-minimal",
        "sim-03-fire-spread",
        "sim-04-fire-multiroom",
    ] {
        write_file(
            &suite.join(app_id).join("main.mei"),
            &format!(
                r#"
app(
    id = "{app_id}",
    default_stage = "home",
)

app.add_scene(
    id = "home",
    profile = "page",
)

scene.set_world(
    resources = [
        resource(id = "welcome_doc", kind = "document", content = "hello"),
    ],
)

scene.set_frame(
    layout = flex(direction = "column"),
)

frame.add_panel(
    id = "welcome",
    area = "auto",
    blocks = [
        doc.markdown(area = "auto", resource = resource_ref("welcome_doc")),
    ],
)
"#
            ),
        );
    }
    write_file(
        &suite.join("cockpit-02-multi-entry").join("default.mei"),
        r#"
scene(
    id = "default_compare",
    profile = "page",
)

frame(
    id = "default_compare_frame",
    layout = flex(direction = "column"),
)
"#,
    );
    root
}
