#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn ensure_materialize_fills_missing_authoring_tree() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp =
            std::env::temp_dir().join(format!("mei-ensure-stock-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp workspace");
        let report = ensure_workspace_stock_materialized(temp.as_path(), package_root.as_path())
            .expect("ensure stock")
            .expect("should materialize");
        assert!(report.authoring.copied_files > 0);
        assert!(
            ensure_workspace_stock_materialized(temp.as_path(), package_root.as_path())
                .expect("ensure again")
                .is_none()
        );
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn ensure_skips_when_bootstrap_refresh_false() {
        let temp = std::env::temp_dir().join(format!(
            "mei-ensure-stock-no-refresh-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        let package_root = temp.join("package");
        let workspace_root = temp.join("workspace");
        let src = package_root.join("stock/components/demo/marker.txt");
        fs::create_dir_all(src.parent().expect("parent")).expect("create src dir");
        fs::create_dir_all(package_root.join("stock/templates")).expect("templates src");
        fs::create_dir_all(package_root.join("stock/authoring")).expect("authoring src");
        fs::write(package_root.join("stock/templates/.keep"), "").expect("templates keep");
        fs::write(package_root.join("stock/authoring/.keep"), "").expect("authoring keep");
        fs::write(&src, "platform").expect("write src");
        materialize_workspace_stock(workspace_root.as_path(), package_root.as_path(), false)
            .expect("initial materialize");
        let dest = workspace_root.join("stock/components/demo/marker.txt");
        fs::remove_file(&dest).expect("delete workspace copy");
        fs::write(
            workspace_root.join("workspace.json"),
            r#"{"schemaVersion":2,"stock":{"bootstrap":{"refresh":false}}}"#,
        )
        .expect("write workspace.json");
        assert!(
            ensure_workspace_stock_materialized(workspace_root.as_path(), package_root.as_path())
                .expect("ensure with refresh false")
                .is_none()
        );
        assert!(
            !dest.is_file(),
            "deleted file must not be restored when refresh=false"
        );
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn ensure_materialize_refreshes_when_platform_source_is_newer() {
        let temp = std::env::temp_dir().join(format!(
            "mei-ensure-stock-refresh-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        let package_root = temp.join("package");
        let workspace_root = temp.join("workspace");
        let src = package_root.join("stock/components/demo/refresh-marker.txt");
        fs::create_dir_all(src.parent().expect("parent")).expect("create src dir");
        fs::write(&src, "v1").expect("write src v1");
        materialize_workspace_stock(workspace_root.as_path(), package_root.as_path(), false)
            .expect("initial materialize");
        let dest = workspace_root.join("stock/components/demo/refresh-marker.txt");
        assert_eq!(fs::read_to_string(&dest).expect("read dest"), "v1");
        std::process::Command::new("touch")
            .args(["-t", "202001010000", &dest.to_string_lossy()])
            .status()
            .expect("touch dest to past");
        fs::write(&src, "v2").expect("write src v2");
        let report =
            ensure_workspace_stock_materialized(workspace_root.as_path(), package_root.as_path())
                .expect("ensure refresh")
                .expect("should refresh newer platform file");
        assert!(
            report.components.copied_files > 0,
            "expected copied_files > 0 when platform source is newer"
        );
        assert_eq!(fs::read_to_string(&dest).expect("read refreshed"), "v2");
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn materialize_report_includes_authoring_tree() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp =
            std::env::temp_dir().join(format!("mei-materialize-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("temp dir");
        let report = materialize_workspace_stock(temp.as_path(), package_root.as_path(), true)
            .expect("materialize");
        assert!(
            temp.join("stock/authoring/README.md").is_file(),
            "authoring stub README should be copied"
        );
        assert_eq!(report.authoring.copied_files > 0, true);
        let json = serde_json::to_value(&report).expect("serialize");
        assert!(
            json.get("authoring").is_some(),
            "json must include authoring"
        );
        assert!(
            temp.join("stock/STOCK.json").is_file(),
            "STOCK.json manifest should be written"
        );
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn doctor_detects_missing_tree() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp =
            std::env::temp_dir().join(format!("mei-doctor-stock-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp workspace");
        let report =
            doctor_workspace_stock(temp.as_path(), package_root.as_path()).expect("doctor");
        assert!(!report.ok, "empty workspace should not pass doctor");
        assert_eq!(report.missing_trees.len(), 3);
        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn workspace_stock_revision_reads_manifest_fingerprint() {
        let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let temp =
            std::env::temp_dir().join(format!("mei-stock-revision-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).expect("create temp workspace");
        materialize_workspace_stock(temp.as_path(), package_root.as_path(), false)
            .expect("materialize");
        let revision = workspace_stock_revision(temp.as_path()).expect("revision");
        assert!(
            revision.starts_with("stock-v"),
            "unexpected revision format: {revision}"
        );
        let _ = fs::remove_dir_all(&temp);
    }
}
