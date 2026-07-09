use mei_lang_kernel::{CompiledApp, Diagnostic};

pub(crate) const MANAGE_PIPELINE_DIAG_CODE: &str = "manage_page_pipeline";

pub(crate) fn is_manage_pipeline_diag(diag: &Diagnostic) -> bool {
    diag.code == MANAGE_PIPELINE_DIAG_CODE
}

pub(crate) fn is_compile_diagnostic(diag: &Diagnostic) -> bool {
    !is_manage_pipeline_diag(diag)
}

pub(crate) fn normalize_target_path(target: &str) -> String {
    target
        .trim()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string()
}

pub(crate) fn is_world_capsule_target(target: &str) -> bool {
    normalize_target_path(target).ends_with(".world.mei")
}

pub(crate) fn world_capsule_companion_scene(target: &str) -> Option<String> {
    normalize_target_path(target)
        .strip_suffix(".world.mei")
        .map(|base| format!("{base}.mei"))
}

pub(crate) fn normalize_diagnostic_source(
    app_root: &str,
    source_path: Option<&str>,
) -> Option<String> {
    let raw = source_path?.trim();
    if raw.is_empty() {
        return None;
    }
    let mut path = raw.replace('\\', "/");
    let root = app_root
        .trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();
    if !root.is_empty() {
        if let Some(stripped) = path.strip_prefix(&root) {
            path = stripped.trim_start_matches('/').to_string();
        } else if path.starts_with(&format!("{root}/")) {
            path = path[root.len() + 1..].to_string();
        }
    }
    let path = path.trim_start_matches("./").to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

pub(crate) fn diagnostic_matches_target(
    compiled: &CompiledApp,
    selected_target: &str,
    diag: &Diagnostic,
) -> bool {
    if !is_compile_diagnostic(diag) {
        return false;
    }
    let target = normalize_target_path(selected_target);
    let Some(source) = normalize_diagnostic_source(&compiled.app_root, diag.source_path.as_deref())
    else {
        return false;
    };
    if source == target {
        return true;
    }
    if source.ends_with(&format!("/{target}")) {
        return true;
    }
    let target_base = target.rsplit('/').next().unwrap_or(target.as_str());
    let source_base = source.rsplit('/').next().unwrap_or(source.as_str());
    !target_base.is_empty() && target_base == source_base
}

fn is_world_capsule_manage_hint(
    compiled: &CompiledApp,
    selected_target: &str,
    diag: &Diagnostic,
) -> bool {
    is_world_capsule_target(selected_target)
        && diagnostic_matches_target(compiled, selected_target, diag)
        && matches!(
            diag.code.as_str(),
            "missing_scene" | "public_fragment_file_deprecated"
        )
}

pub(crate) fn should_display_diagnostic(
    compiled: &CompiledApp,
    selected_target: &str,
    diag: &Diagnostic,
) -> bool {
    !is_world_capsule_manage_hint(compiled, selected_target, diag)
}

/// 预览降级：优先当前文件 Error，不足时再补其它文件 Error。
pub(crate) fn blocking_errors_for_preview<'a>(
    compiled: &'a CompiledApp,
    selected_target: &str,
    limit: usize,
) -> Vec<&'a Diagnostic> {
    let mut picked = Vec::new();
    for diag in compiled.diagnostics.iter().filter(|diag| {
        matches!(diag.severity, mei_lang_kernel::Severity::Error)
            && diagnostic_matches_target(compiled, selected_target, diag)
            && should_display_diagnostic(compiled, selected_target, diag)
    }) {
        picked.push(diag);
        if picked.len() >= limit {
            return picked;
        }
    }
    for diag in compiled.diagnostics.iter().filter(|diag| {
        matches!(diag.severity, mei_lang_kernel::Severity::Error)
            && !diagnostic_matches_target(compiled, selected_target, diag)
            && should_display_diagnostic(compiled, selected_target, diag)
    }) {
        picked.push(diag);
        if picked.len() >= limit {
            break;
        }
    }
    picked
}

#[cfg(test)]
mod tests {
    use mei_lang_kernel::{Diagnostic, Severity};

    use super::*;

    fn diag(code: &str, path: &str) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code: code.to_string(),
            message: "msg".to_string(),
            source_path: Some(path.to_string()),
        }
    }

    fn sample_compiled(diags: Vec<Diagnostic>) -> CompiledApp {
        CompiledApp {
            app_id: "demo".to_string(),
            title: "demo".to_string(),
            app_root: "/tmp/app".to_string(),
            scene_routes: Vec::new(),
            active_scene: None,
            active_target_file: "main.mei".to_string(),
            file_tree: Vec::new(),
            scene_contract: None,
            scene_local_nav_by_target: std::collections::BTreeMap::new(),
            scene_bindings_by_id: std::collections::BTreeMap::new(),
            scene_examples_by_id: std::collections::BTreeMap::new(),
            scene_projection_assembly_by_id: std::collections::BTreeMap::new(),
            resources: Vec::new(),
            world_metrics: std::collections::BTreeMap::new(),
            world_semantic_by_file: std::collections::BTreeMap::new(),
            component_assets: Vec::new(),
            diagnostics: diags,
            build_experience_index: Default::default(),
            build_t2_page_index: Default::default(),
            build_template_index: Default::default(),
            ui_layout_index: Default::default(),
        }
    }

    #[test]
    fn diagnostic_matches_target_by_relative_path() {
        let compiled = sample_compiled(vec![diag("missing_scene", "panels/shared-frame.mei")]);
        assert!(diagnostic_matches_target(
            &compiled,
            "panels/shared-frame.mei",
            &compiled.diagnostics[0]
        ));
        assert!(!diagnostic_matches_target(
            &compiled,
            "main.mei",
            &compiled.diagnostics[0]
        ));
    }

    #[test]
    fn compile_diagnostics_filter_current_file() {
        let compiled = sample_compiled(vec![
            diag("missing_scene", "panels/shared-frame.mei"),
            diag("missing_scene", "main.mei"),
        ]);
        let current: Vec<_> = compiled
            .diagnostics
            .iter()
            .filter(|diag| {
                diagnostic_matches_target(&compiled, "panels/shared-frame.mei", diag)
                    && should_display_diagnostic(&compiled, "panels/shared-frame.mei", diag)
            })
            .collect();
        assert_eq!(current.len(), 1);
        assert_eq!(
            current[0].source_path.as_deref(),
            Some("panels/shared-frame.mei")
        );
    }

    #[test]
    fn world_capsule_manage_hints_are_filtered_from_current_target() {
        let compiled = sample_compiled(vec![
            Diagnostic {
                severity: Severity::Warning,
                code: "public_fragment_file_deprecated".to_string(),
                message: "msg".to_string(),
                source_path: Some("scenes/foo.world.mei".to_string()),
            },
            diag("missing_scene", "scenes/foo.world.mei"),
            diag("invalid_resource_ref", "scenes/foo.world.mei"),
        ]);
        let current: Vec<_> = compiled
            .diagnostics
            .iter()
            .filter(|diag| {
                diagnostic_matches_target(&compiled, "scenes/foo.world.mei", diag)
                    && should_display_diagnostic(&compiled, "scenes/foo.world.mei", diag)
            })
            .collect();
        assert_eq!(current.len(), 1);
        assert_eq!(current[0].code, "invalid_resource_ref");
        assert!(compiled.diagnostics.iter().any(|diag| {
            matches!(diag.severity, Severity::Error)
                && should_display_diagnostic(&compiled, "scenes/foo.world.mei", diag)
        }));
    }
}
