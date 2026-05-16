use std::path::Path;

use serde_json::Value;

use crate::model::{AppDecl, Diagnostic, Severity};

pub(super) fn decode_app_decl(path: &Path, raw: &Value) -> (Option<AppDecl>, Vec<Diagnostic>) {
    let mut app_decl = None;
    let mut diagnostics = Vec::new();
    let mut app_decl_count = 0usize;
    if let Some(values) = raw.as_array() {
        for value in values {
            if value.get("kind").and_then(Value::as_str) == Some("app") {
                app_decl_count += 1;
                match serde_json::from_value::<AppDecl>(value.clone()) {
                    Ok(decl) => {
                        if app_decl.is_none() {
                            app_decl = Some(decl);
                        }
                    }
                    Err(error) => diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        code: "decode_app_failed".to_string(),
                        message: error.to_string(),
                        source_path: Some(path.to_string_lossy().to_string()),
                    }),
                }
            }
        }
    }
    if app_decl_count > 1 {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: "multiple_apps".to_string(),
            message: format!(
                "file `{}` declares {app_decl_count} app(...) blocks, expected exactly one",
                path.display()
            ),
            source_path: Some(path.to_string_lossy().to_string()),
        });
    }
    (app_decl, diagnostics)
}
