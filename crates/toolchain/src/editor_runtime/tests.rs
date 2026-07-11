#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use mei_lang_kernel::{RuntimeWarmupManifest, WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL};

    use super::prelude::*;
    use super::*;

    fn temp_workspace_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("mei-editor-runtime-{name}-{nanos}"))
    }

    #[test]
    fn install_runtime_writes_warmup_manifest() {
        let workspace_root = temp_workspace_root("warmup");
        let app_root = workspace_root.join("demo");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::write(
            app_root.join("main.mei"),
            "app(id=\"demo\")\nscene(id=\"home\", target=\"home.mei\")\n",
        )
        .expect("write main");
        fs::write(app_root.join("home.mei"), "frame()").expect("write scene");
        fs::write(
            workspace_root.join(".mei-workspace.json"),
            r#"{
  "warmup": {
    "apps": {
      "demo": {
        "hotScenes": ["command-center"],
        "datasets": [
          {
            "sceneId": "home",
            "datasetId": "warning_list",
            "metricId": "case_total"
          }
        ]
      }
    }
  }
}"#,
        )
        .expect("write workspace config");

        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("mei-lang package root")
            .to_path_buf();
        install_editor_runtime_support_files(&workspace_root, &package_root, true)
            .expect("install runtime");

        let manifest_path = workspace_root.join(WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL);
        let raw = fs::read_to_string(&manifest_path).expect("read manifest");
        let manifest: RuntimeWarmupManifest =
            serde_json::from_str(&raw).expect("parse warmup manifest");
        assert!(manifest.enabled);
        assert_eq!(manifest.apps.len(), 1);
        assert_eq!(manifest.apps[0].app_id, "demo");
        assert_eq!(
            manifest.apps[0].hot_scenes,
            vec!["command-center".to_string()]
        );
        assert!(
            manifest.apps[0]
                .scenes
                .contains(&"command-center".to_string()),
            "expected hot scene to be included in merged warmup scenes"
        );
        assert_eq!(manifest.apps[0].datasets.len(), 1);
        assert_eq!(manifest.apps[0].datasets[0].dataset_id, "warning_list");
        assert_eq!(
            manifest.apps[0].datasets[0].metric_id.as_deref(),
            Some("case_total")
        );
        assert_eq!(manifest.apps[0].focuses, vec!["main.mei".to_string()]);
        assert!(manifest.apps[0].datasets[0].focus.is_none());

        let _ = fs::remove_dir_all(&workspace_root);
    }

    #[test]
    fn ensure_author_skill_installs_when_missing_without_full_runtime_install() {
        let workspace_root = temp_workspace_root("ensure-author-skill");
        fs::create_dir_all(&workspace_root).expect("create workspace root");
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("mei-lang package root")
            .to_path_buf();
        assert!(
            !workspace_root
                .join("runtime/platform/skills/meilang-author/SKILL.md")
                .is_file(),
            "fixture should start without author skill"
        );
        let report = ensure_workspace_author_skill_package(&workspace_root, &package_root)
            .expect("ensure author skill");
        assert!(report.installed);
        assert!(report.installed_now);
        assert!(report.file_count > 0);
        assert!(workspace_root
            .join("runtime/platform/skills/meilang-author/SKILL.md")
            .is_file());
        let again = ensure_workspace_author_skill_package(&workspace_root, &package_root)
            .expect("ensure author skill again");
        assert!(again.installed);
        assert!(!again.installed_now);
        let _ = fs::remove_dir_all(&workspace_root);
    }
}
