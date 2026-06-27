use mei_lang_kernel::evaluate_mei_file;
use mei_lower::lower_path;
use serde_json::Value as JsonValue;

use mei_compiler_tests::{assert_decl_ir_eq, ws_hello_hello_app_dir};

#[test]
fn hello_main_matches_starlark() {
    let path = ws_hello_hello_app_dir().join("src/main.mei");
    let starlark = evaluate_mei_file(&path).expect("starlark main.mei");
    let outcome = lower_path(&path).expect("native main.mei");
    let native = JsonValue::Array(outcome.exports);
    assert_decl_ir_eq(&starlark, &native);
}

#[test]
fn hello_home_matches_starlark() {
    let path = ws_hello_hello_app_dir().join("src/scenes/home.mei");
    let starlark = evaluate_mei_file(&path).expect("starlark home.mei");
    let outcome = lower_path(&path).expect("native home.mei");
    let native = JsonValue::Array(outcome.exports);
    assert_decl_ir_eq(&starlark, &native);
}
