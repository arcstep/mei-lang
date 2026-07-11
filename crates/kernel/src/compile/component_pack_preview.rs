use std::fs;
use std::path::Path;

use crate::model::{CompiledApp, ComponentAsset};

pub fn component_pack_preview_workspace_path(asset: &ComponentAsset) -> Option<String> {
    asset.preview_mei.clone()
}

pub fn component_pack_preview_relative_to_app(
    compiled: &CompiledApp,
    asset: &ComponentAsset,
) -> Option<String> {
    let workspace_path = component_pack_preview_workspace_path(asset)?;
    let rel =
        super::build_experience::preview_target_relative_to_app(compiled, workspace_path.as_str())?;
    if !pack_preview_supports_authoring_compile(compiled, rel.as_str()) {
        return None;
    }
    Some(rel)
}

pub fn component_pack_preview_relative_to_app_for_key(
    compiled: &CompiledApp,
    use_key: &str,
) -> Option<String> {
    let source_root = crate::mei_config::resolve_workspace_source_root_from_app_root(Path::new(
        compiled.app_root.as_str(),
    ));
    let asset = crate::workspace::load_component_assets(source_root.as_path())
        .ok()?
        .remove(use_key)?;
    component_pack_preview_relative_to_app(compiled, &asset)
}

fn pack_preview_supports_authoring_compile(compiled: &CompiledApp, rel_path: &str) -> bool {
    let app_root = Path::new(compiled.app_root.as_str());
    let abs = if rel_path.starts_with("../") {
        let mut base = app_root.to_path_buf();
        for part in rel_path.split('/') {
            if part == ".." {
                if !base.pop() {
                    return false;
                }
            } else if !part.is_empty() && part != "." {
                base.push(part);
            }
        }
        base
    } else {
        app_root.join(rel_path)
    };
    let content = match fs::read_to_string(abs) {
        Ok(value) => value,
        Err(_) => return false,
    };
    let trimmed = content.trim();
    !trimmed.is_empty() && trimmed.contains("scene(")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn pack_preview_path_comes_from_asset_preview_mei() {
        let asset = ComponentAsset {
            key: "chart.donut".to_string(),
            tag: "mei-chart-donut".to_string(),
            script: "chart/echarts/donut.js".to_string(),
            pack_path: "chart/echarts".to_string(),
            preview_mei: Some(
                "stock/components/chart/echarts/previews/chart.donut.mei".to_string(),
            ),
        };
        assert_eq!(
            component_pack_preview_workspace_path(&asset).as_deref(),
            Some("stock/components/chart/echarts/previews/chart.donut.mei")
        );
    }

    #[test]
    fn audit_reports_missing_pack_previews() {
        let root = std::env::temp_dir().join(format!("mei-audit-preview-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("stock/components/chart/echarts")).expect("mkdir");
        fs::write(
            root.join("stock/components/chart/echarts/manifest.json"),
            r#"{"components":{"chart.line":{"tag":"mei-chart-line","script":"line.js"}}}"#,
        )
        .expect("manifest");
        fs::write(root.join("stock/components/chart/echarts/line.js"), "//").expect("script");
        let missing =
            crate::workspace::audit_component_preview_coverage(root.as_path()).expect("audit");
        assert_eq!(missing, vec!["chart.line".to_string()]);
        let _ = fs::remove_dir_all(&root);
    }
}
