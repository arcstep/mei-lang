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

pub(super) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("workspace root")
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
    default_scene = "home",
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
        doc.markdown(area = "auto", resource = world_ref("welcome_doc")),
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
