use super::*;
use super::io::{write_mei_config, write_workspace_config};
use super::types::{AppEntryConfig, AuthUserConfig, DiscoverConfig, MeiConfig, WorkspaceAuthConfig, WorkspaceConfig, MEI_CONFIG_FILENAME};
use super::auth_bundle::workspace_auth_config_path;
use super::workspace_paths::workspace_config_path;

    #[test]
    fn workspace_compliance_deserializes_from_json() {
        let raw = r#"{
            "compliance": {
                "icpRecord": "渝ICP备12345678号",
                "psbRecord": "渝公网安备 12345678号",
                "copyright": "示例主体"
            }
        }"#;
        let cfg: WorkspaceConfig = serde_json::from_str(raw).expect("parse compliance");
        assert_eq!(
            cfg.compliance.icp_record_trimmed(),
            Some("渝ICP备12345678号")
        );
        assert_eq!(
            cfg.compliance.psb_record_trimmed(),
            Some("渝公网安备 12345678号")
        );
        assert_eq!(cfg.compliance.copyright_trimmed(), Some("示例主体"));
    }

    #[test]
    fn workspace_discover_skip_normalizes_segments() {
        let cfg = WorkspaceConfig {
            discover: DiscoverConfig {
                skip_directories: vec![" /foo/ ".into(), "nested/bad".into(), "ok".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(cfg.discover_skip_directories(), vec!["foo", "ok"]);
    }

    #[test]
    fn entry_main_defaults_to_main_mei() {
        let entry = AppEntryConfig::default();
        assert_eq!(entry.main_rel(), "main.mei");
        let entry = AppEntryConfig {
            main: " scenes/home.mei ".into(),
        };
        assert_eq!(entry.main_rel(), "scenes/home.mei");
    }

    #[test]
    fn workspace_auth_bundle_reads_workspace_json() {
        let dir = std::env::temp_dir().join(format!(
            "mei-auth-bundle-workspace-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("tmpdir");
        let workspace = WorkspaceConfig {
            auth: WorkspaceAuthConfig {
                jwt_secret: Some("workspace-secret".to_string()),
                users: vec![AuthUserConfig {
                    username: "guest01".to_string(),
                    password_hash: "$argon2id$v=19$workspace".to_string(),
                    roles: vec!["guest".to_string()],
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..Default::default()
        };
        write_workspace_config(&workspace_config_path(&dir), &workspace).expect("write workspace");
        let bundle = load_workspace_auth_bundle(&dir);
        assert_eq!(bundle.auth.jwt_secret.as_deref(), Some("workspace-secret"));
        assert_eq!(bundle.auth.users.len(), 1);
        assert_eq!(bundle.auth.users[0].username, "guest01");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_auth_bundle_reads_misplaced_mei_config_for_migration() {
        let dir = std::env::temp_dir().join(format!(
            "mei-auth-bundle-misplaced-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("tmpdir");
        let mut config = MeiConfig::default();
        config.auth.jwt_secret = Some("misplaced-secret".to_string());
        config.auth.users.push(AuthUserConfig {
            username: "admin".to_string(),
            password_hash: "$argon2id$v=19$misplaced".to_string(),
            roles: vec!["admin".to_string()],
            ..Default::default()
        });
        write_mei_config(&dir.join(MEI_CONFIG_FILENAME), &config).expect("seed misplaced mei config");
        let bundle = load_workspace_auth_bundle(&dir);
        assert_eq!(bundle.auth.jwt_secret.as_deref(), Some("misplaced-secret"));
        assert_eq!(bundle.auth.users.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_auth_bundle_writes_workspace_json_without_dropping_runtime() {
        let dir = std::env::temp_dir().join(format!(
            "mei-auth-bundle-write-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("tmpdir");
        let workspace = WorkspaceConfig {
            discover: DiscoverConfig {
                skip_directories: vec!["cache".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        write_workspace_config(&workspace_auth_config_path(&dir), &workspace)
            .expect("seed workspace config");
        let mut auth = WorkspaceAuthConfig::default();
        auth.jwt_secret = Some("jwt".to_string());
        auth.users.push(AuthUserConfig {
            username: "admin".to_string(),
            password_hash: "$argon2id$v=19$demo".to_string(),
            roles: vec!["admin".to_string()],
            ..Default::default()
        });
        write_workspace_auth_bundle(&dir, &auth).expect("write auth");
        let loaded = WorkspaceConfig::load_or_default(&workspace_auth_config_path(&dir));
        assert_eq!(loaded.discover.skip_directories, vec!["cache"]);
        assert_eq!(loaded.auth.users.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
