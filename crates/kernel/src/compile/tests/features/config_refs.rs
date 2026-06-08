use super::{compile_app_from_root_with_options, temp_root, write_file, CompileOptions};
use crate::model::Severity;

#[test]
fn missing_source_ref_emits_compile_diagnostic() {
    let source_root = temp_root("config-ref-missing");
    let app_root = source_root.join("demo");
    write_file(
        &app_root.join(".mei-config.json"),
        r#"{"schemaVersion":1,"ops":{"sources":{}}}"#,
    );
    write_file(
        &app_root.join("main.mei"),
        r#"
app(id = "demo", scene = scene(id = "s1"))

world(
    datasets = [
        resource(
            id = "rows",
            kind = "dataset",
            source = source_ref("missing"),
            dataset = dataset(id = "rows", columns = []),
        ),
    ],
)
"#,
    );

    let compiled =
        compile_app_from_root_with_options(&source_root, &app_root, CompileOptions::default())
            .expect("compile");
    assert!(
        compiled
            .diagnostics
            .iter()
            .any(|diag| { diag.code == "missing_config_ref" && diag.severity == Severity::Error }),
        "expected missing_config_ref diagnostic: {:?}",
        compiled.diagnostics
    );
    let _ = std::fs::remove_dir_all(&source_root);
}
