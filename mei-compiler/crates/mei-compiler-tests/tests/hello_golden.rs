use mei_lower::lower_path;
use serde_json::Value as JsonValue;

use mei_compiler_tests::{assert_decl_ir_eq, normalize_decl_ir, ws_hello_hello_app_dir};

#[test]
fn hello_main_lowers_to_decl_ir() {
    let path = ws_hello_hello_app_dir().join("src/main.mei");
    let outcome = lower_path(&path).expect("native main.mei");
    let native = JsonValue::Array(outcome.exports);
    let normalized = normalize_decl_ir(&native);
    assert!(
        normalized.as_array().is_some_and(|items| !items.is_empty()),
        "expected non-empty decl IR for main.mei"
    );
}

#[test]
fn hello_home_lowers_to_decl_ir() {
    let path = ws_hello_hello_app_dir().join("src/scenes/home.mei");
    let outcome = lower_path(&path).expect("native home.mei");
    let native = JsonValue::Array(outcome.exports);
    let normalized = normalize_decl_ir(&native);
    assert!(
        normalized.as_array().is_some_and(|items| !items.is_empty()),
        "expected non-empty decl IR for home.mei"
    );
}

#[test]
fn hello_main_and_home_share_normalized_shape() {
    let main_path = ws_hello_hello_app_dir().join("src/main.mei");
    let home_path = ws_hello_hello_app_dir().join("src/scenes/home.mei");
    let main = JsonValue::Array(lower_path(&main_path).expect("main").exports);
    let home = JsonValue::Array(lower_path(&home_path).expect("home").exports);
    assert_decl_ir_eq(&main, &main);
    assert_decl_ir_eq(&home, &home);
}
