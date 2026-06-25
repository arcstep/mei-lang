use super::auth_bundle::{workspace_auth_config_path, workspace_auth_host_id};
use super::io::{write_mei_config, write_workspace_config};
use super::types::{
    AppEntryConfig, AuthUserConfig, DiscoverConfig, MeiConfig, WorkspaceAuthConfig,
    WorkspaceConfig, WorkspaceHostState, WorkspaceWarmupDatasetConfig, MEI_CONFIG_FILENAME,
};
use super::workspace_paths::workspace_config_path;
use super::*;

#[test]
fn access_ai_external_deserializes_from_features() {
    let raw = r#"{
        "features": {
            "aiChat": false,
            "accessAiExternal": {
                "url": "https://example.test/agent",
                "image": "/workspace-app-assets/demo/assets/AI@3x.png",
                "label": "Demo AI",
                "openInNewTab": true
            }
        }
    }"#;
    let cfg: MeiConfig = serde_json::from_str(raw).expect("parse accessAiExternal");
    let external = cfg
        .features
        .access_ai_external
        .as_ref()
        .expect("accessAiExternal");
    assert_eq!(external.url, "https://example.test/agent");
    assert_eq!(
        external.image,
        "/workspace-app-assets/demo/assets/AI@3x.png"
    );
    assert!(external.is_configured());
    assert!(external.open_in_new_tab());
    assert_eq!(external.label_or_default(), "Demo AI");
}

#[test]
fn workspace_default_app_deserializes_from_json() {
    let raw = r#"{
            "workspace": {
                "id": "ws-demo",
                "defaultApp": "zhifa"
            }
        }"#;
    let cfg: WorkspaceConfig = serde_json::from_str(raw).expect("parse defaultApp");
    assert_eq!(
        cfg.workspace.default_app.as_deref(),
        Some("zhifa")
    );
}

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
fn workspace_warmup_deserializes_from_json() {
    let raw = r#"{
            "warmup": {
                "apps": {
                    "zhifa": {
                        "hotScenes": ["home", "  command-center  "],
                        "datasets": [
                            {
                                "sceneId": "home",
                                "datasetId": "warning_list"
                            },
                            {
                                "sceneId": "home",
                                "datasetId": "warning_metric",
                              "metricId": "case_total"
                            },
                            {
                              "sceneId": "home",
                              "datasetId": "warning_batch",
                              "metricIds": ["case_total", "case_delta"]
                            }
                        ]
                    }
                }
            }
        }"#;
    let cfg: WorkspaceConfig = serde_json::from_str(raw).expect("parse warmup");
    let zhifa = cfg.warmup.apps.get("zhifa").expect("zhifa warmup config");
    assert_eq!(zhifa.hot_scenes.len(), 2);
    assert_eq!(
        zhifa.datasets,
        vec![
            WorkspaceWarmupDatasetConfig {
                scene_id: Some("home".to_string()),
                focus: None,
                dataset_id: "warning_list".to_string(),
                priority: None,
                metric_id: None,
                metric_ids: Vec::new(),
            },
            WorkspaceWarmupDatasetConfig {
                scene_id: Some("home".to_string()),
                focus: None,
                dataset_id: "warning_metric".to_string(),
                priority: None,
                metric_id: Some("case_total".to_string()),
                metric_ids: Vec::new(),
            },
            WorkspaceWarmupDatasetConfig {
                scene_id: Some("home".to_string()),
                focus: None,
                dataset_id: "warning_batch".to_string(),
                priority: None,
                metric_id: None,
                metric_ids: vec!["case_total".to_string(), "case_delta".to_string()],
            }
        ]
    );
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
    assert_eq!(bundle.loaded_from, "workspace_config_auth");
    assert_eq!(bundle.workspace_config_path, workspace_config_path(&dir));
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
    assert_eq!(bundle.loaded_from, "legacy_mei_config_auth");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn workspace_auth_bundle_writes_host_state_and_scrubs_workspace_auth() {
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
    write_workspace_config(&workspace_config_path(&dir), &workspace)
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
    let loaded = WorkspaceConfig::load_or_default(&workspace_config_path(&dir));
    assert_eq!(loaded.discover.skip_directories, vec!["cache"]);
    assert!(loaded.auth.is_empty());
    let state = WorkspaceHostState::load_or_default(&workspace_auth_config_path(&dir));
    assert_eq!(state.host_id.as_deref(), Some(DEFAULT_HOST_STATE_ID));
    assert_eq!(state.auth.users.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn workspace_auth_bundle_prefers_host_state_over_workspace_json() {
    let dir = std::env::temp_dir().join(format!(
        "mei-auth-bundle-state-preferred-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("tmpdir");
    let workspace = WorkspaceConfig {
        auth: WorkspaceAuthConfig {
            jwt_secret: Some("workspace-secret".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    write_workspace_config(&workspace_config_path(&dir), &workspace).expect("write workspace");
    let state = WorkspaceHostState {
        schema_version: WORKSPACE_HOST_STATE_SCHEMA_VERSION,
        host_id: Some(DEFAULT_HOST_STATE_ID.to_string()),
        auth: WorkspaceAuthConfig {
            jwt_secret: Some("state-secret".to_string()),
            ..Default::default()
        },
    };
    let raw = serde_json::to_string_pretty(&state).expect("serialize state");
    std::fs::create_dir_all(
        workspace_auth_config_path(&dir)
            .parent()
            .expect("state parent"),
    )
    .expect("state dir");
    std::fs::write(workspace_auth_config_path(&dir), raw).expect("write state");
    let bundle = load_workspace_auth_bundle(&dir);
    assert_eq!(bundle.loaded_from, "workspace_host_state");
    assert_eq!(bundle.auth.jwt_secret.as_deref(), Some("state-secret"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn workspace_auth_path_uses_deploy_host_when_present() {
    let dir = std::env::temp_dir().join(format!(
        "mei-auth-host-id-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("tmpdir");
    let workspace = WorkspaceConfig {
        workspace: WorkspaceProfile {
            deploy_host: Some("zw-spbjw".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    write_workspace_config(&workspace_config_path(&dir), &workspace).expect("write workspace");
    assert_eq!(workspace_auth_host_id(&dir), "zw-spbjw");
    assert!(workspace_auth_config_path(&dir).ends_with("runtime/hosts/zw-spbjw.state.json"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_authoring_helpers_loads_workspace_star_files() {
    let dir = std::env::temp_dir().join(format!(
        "mei-authoring-helpers-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let authoring_dir = dir.join(".stock/authoring");
    std::fs::create_dir_all(&authoring_dir).expect("authoring dir");
    std::fs::write(
        authoring_dir.join("demo.star"),
        "def demo_helper():\n    return [\"a\"]\n",
    )
    .expect("write helper");
    let helpers = resolve_authoring_helpers(&dir).expect("resolve helpers");
    assert!(helpers
        .public_functions
        .contains(&"demo_helper".to_string()));
    assert!(!helpers.fingerprint.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
