use std::fs;

use mei_graph::compile_app;

#[test]
fn compiles_objects_mei_into_object_catalog_graph_block() {
    let temp = tempfile::tempdir().expect("temp workspace");
    let root = temp.path();
    let source_dir = root.join("apps/demo/src/domain");
    fs::create_dir_all(&source_dir).expect("source directory");
    fs::create_dir_all(root.join("stock/templates")).expect("template directory");
    fs::write(
        root.join("workspace.json"),
        r#"{"paths":{"templates":"stock/templates"}}"#,
    )
    .expect("workspace config");
    fs::write(
        source_dir.join("warnings.objects.mei"),
        r#"
object_catalog(
    id = "warning_objects",
    types = [
        object_type(
            id = "zhifa.Warning",
            identity = object_identity(
                fields = ["warning_id"],
                aliases = ["warningId"],
            ),
            source = dataset_ref("warning_rows"),
            capabilities = ["select", "explain"],
        ),
    ],
)
"#,
    )
    .expect("object catalog source");

    let outcome = compile_app(root, "demo").expect("compile object catalog");
    let source = outcome
        .files
        .iter()
        .find(|file| file.source_file == "domain/warnings.objects.mei")
        .expect(".objects.mei source outcome");
    let block = source
        .blocks
        .iter()
        .find(|block| block.kind == "object_catalog")
        .expect("object_catalog GraphBlock");

    assert_eq!(block.schema, "mei-object-catalog-v1");
    assert_eq!(block.block_id, "object_catalog:warning_objects");
    assert_eq!(block.payload["types"][0]["id"], "zhifa.Warning");
    assert_eq!(
        block.payload["types"][0]["identity"]["aliases"][0],
        "warningId"
    );
    assert_eq!(block.payload["types"][0]["capabilities"][0], "select");
    assert_eq!(
        block.payload["source_anchor"],
        "domain/warnings.objects.mei"
    );
}
