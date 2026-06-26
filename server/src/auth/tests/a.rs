use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    middleware,
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use mei_lang_kernel::{load_workspace_auth_bundle, AuthJournal};
use rsa::{pkcs8::DecodePublicKey, Oaep, RsaPublicKey};
use sha2::Sha256;
use tower::ServiceExt;

use crate::{agent_runtime, mei_agent, AppState, SessionContextSnapshot};

use super::authorize::{
    auth_middleware, authorize_next_path, authorize_path, format_auth_not_ready_message,
    prepare_auth_for_serve,
};
use super::crypto::{
    decrypt_base64_with_private_key, generate_key_pair_pem, generate_temporary_password,
    hash_password, validate_password_complexity,
};
use super::runtime::load_auth_runtime;
use super::types::{AuthEnforcement, AuthPrincipal, AuthRole};
use super::workspace_users::{ensure_workspace_auth_base, upsert_workspace_user};

fn temp_source_root(label: &str) -> PathBuf {
    let mut dir = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    dir.push(format!("mei-auth-test-{label}-{stamp}"));
    fs::create_dir_all(&dir).expect("create temp source root");
    dir
}

fn make_state(source_root: PathBuf, enforcement: AuthEnforcement) -> AppState {
    let package_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("server crate parent")
        .to_path_buf();
    let native_agent =
        Arc::new(mei_agent::NativeAgent::open(source_root.clone()).expect("native agent"));
    AppState {
        package_root: Arc::new(package_root),
        source_root: Arc::new(source_root),
        agent_preferred_mode: Arc::new("native".to_string()),
        agent_preferred_server_url: Arc::new(String::new()),
        agent_auto_start: false,
        auth_enforcement: enforcement,
        agent_runtime: Arc::new(Mutex::new(agent_runtime::ManagedOpencodeRuntime::default())),
        agent_session_context: Arc::new(Mutex::new(
            HashMap::<String, SessionContextSnapshot>::new(),
        )),
        native_agent,
    }
}

fn bootstrap_guest_user(source_root: &Path, app_allow: &[&str]) {
    ensure_workspace_auth_base(source_root).expect("ensure auth base");
    let hash = hash_password("GuestPwd1!safe").expect("hash guest password");
    let app_allow = app_allow
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    upsert_workspace_user(
        source_root,
        "guest01",
        "访客",
        AuthRole::Guest,
        hash.as_str(),
        &app_allow,
        &[],
        &BTreeMap::new(),
    )
    .expect("upsert guest");
}

fn bootstrap_admin_user(source_root: &Path) {
    ensure_workspace_auth_base(source_root).expect("ensure auth base");
    let hash = hash_password("AdminPwd1!safe").expect("hash admin password");
    upsert_workspace_user(
        source_root,
        "admin01",
        "管理员",
        AuthRole::Admin,
        hash.as_str(),
        &[],
        &[],
        &BTreeMap::new(),
    )
    .expect("upsert admin");
}

fn token_for(source_root: &Path, username: &str, password: &str) -> String {
    let runtime = load_auth_runtime(source_root).expect("load runtime");
    let claims = runtime
        .authenticate(username, password)
        .expect("auth result")
        .expect("claims");
    runtime.issue_jwt(&claims).expect("issue jwt")
}

#[test]
fn password_complexity_requires_multiple_char_classes() {
    assert!(validate_password_complexity("Aa1!bcde").is_ok());
    assert!(validate_password_complexity("Aa1!12345678").is_ok());
    assert!(validate_password_complexity("Aa1!bcd").is_err());
    assert!(validate_password_complexity("aaaaaa").is_err());
    assert!(validate_password_complexity("NO_LOWER_123!").is_err());
}

#[test]
fn temporary_password_meets_complexity() {
    let password = generate_temporary_password();
    assert!(validate_password_complexity(password.as_str()).is_ok());
}

#[test]
fn guest_empty_allowlist_allows_workspace_apps_except_denylist() {
    let principal = AuthPrincipal {
        username: "guest".to_string(),
        profile: "访客".to_string(),
        role: AuthRole::Guest,
        app_allowlist: BTreeSet::new(),
        app_denylist: ["blocked".to_string()].into_iter().collect(),
        scene_allowlist: BTreeMap::new(),
        session_exp: 0,
    };
    assert!(principal.can_access_app("demo"));
    assert!(!principal.can_access_app("blocked"));
}

#[test]
fn guest_non_empty_allowlist_restricts_apps() {
    let principal = AuthPrincipal {
        username: "guest".to_string(),
        profile: "访客".to_string(),
        role: AuthRole::Guest,
        app_allowlist: ["demo".to_string()].into_iter().collect(),
        app_denylist: BTreeSet::new(),
        scene_allowlist: BTreeMap::new(),
        session_exp: 0,
    };
    assert!(principal.can_access_app("demo"));
    assert!(!principal.can_access_app("blocked"));
}

fn principal_for_role(role: AuthRole) -> AuthPrincipal {
    AuthPrincipal {
        username: role.as_str().to_string(),
        profile: String::new(),
        role,
        app_allowlist: BTreeSet::new(),
        app_denylist: BTreeSet::new(),
        scene_allowlist: BTreeMap::new(),
        session_exp: 0,
    }
}

fn assert_authorize_path(path: &str, principal: &AuthPrincipal, expect_ok: bool) {
    let result = authorize_path(path, principal);
    if expect_ok {
        assert!(result.is_ok(), "expected allow path={path}: {result:?}");
    } else {
        assert!(result.is_err(), "expected deny path={path}");
    }
}

#[test]
fn authorize_next_path_rejects_build_landing_for_admin() {
    let admin = principal_for_role(AuthRole::Admin);
    assert_eq!(
        authorize_next_path(Some("/apps/build/demo?tab=preview"), &admin),
        "/"
    );
    assert_eq!(
        authorize_next_path(Some("/apps/app/demo/scene/home"), &admin),
        "/apps/app/demo/scene/home"
    );
}

#[test]
fn host_capability_matrix_matches_role_and_authorize_path() {
    use mei_lang_app::HostCapabilities;

    for (role, caps) in [
        (AuthRole::Guest, HostCapabilities::from_role_slug("guest")),
        (AuthRole::Admin, HostCapabilities::from_role_slug("admin")),
        (AuthRole::Super, HostCapabilities::from_role_slug("super")),
    ] {
        let principal = principal_for_role(role);
        assert_eq!(principal.capabilities(), caps);

        assert_authorize_path("/apps/app/demo/scene/home", &principal, caps.access_view);
        assert_authorize_path("/apps/config/demo", &principal, caps.config_upload);
        assert_authorize_path("/apps/upload/demo", &principal, caps.config_upload);
        assert_authorize_path("/apps/build/demo", &principal, caps.build_view);
        assert_authorize_path(
            "/workspace-components/chart/echarts/column.js",
            &principal,
            caps.runtime_components,
        );
        assert_authorize_path("/api/agent/session", &principal, caps.access_agent);
        assert_authorize_path("/api/agent/model/probe", &principal, caps.access_agent);
        assert_authorize_path("/api/agent/start", &principal, caps.agent_control);
        assert_authorize_path(
            "/api/agent/session/s1/revert",
            &principal,
            caps.authoring_agent,
        );
        assert_authorize_path("/api/ops/config/demo", &principal, caps.config_upload);
        assert_authorize_path(
            "/api/upload/download/demo",
            &principal,
            caps.access_view,
        );
        assert_authorize_path("/api/upload/demo", &principal, caps.config_upload);
    }
}

#[test]
fn route_mode_matrix_matches_role_defaults() {
    let guest = AuthPrincipal {
        username: "g".into(),
        profile: String::new(),
        role: AuthRole::Guest,
        app_allowlist: BTreeSet::new(),
        app_denylist: BTreeSet::new(),
        scene_allowlist: BTreeMap::new(),
        session_exp: 0,
    };
    let admin = AuthPrincipal {
        role: AuthRole::Admin,
        ..guest.clone()
    };
    let super_user = AuthPrincipal {
        role: AuthRole::Super,
        ..guest.clone()
    };
    assert!(guest.can_access_host_route_mode("app"));
    assert!(guest.can_access_host_route_mode("presentation"));
    assert!(!guest.can_access_host_route_mode("config"));
    assert!(!guest.can_access_host_route_mode("build"));
    assert!(admin.can_access_host_route_mode("upload"));
    assert!(!admin.can_access_host_route_mode("build"));
    assert!(super_user.can_access_host_route_mode("build"));
}

#[test]
fn rsa_roundtrip_for_sensitive_payload() {
    let (public_pem, private_pem) = generate_key_pair_pem().expect("generate keypair");
    let public = RsaPublicKey::from_public_key_pem(public_pem.as_str()).expect("public key");
    let mut rng = rand::rngs::OsRng;
    let encrypted = public
        .encrypt(&mut rng, Oaep::new::<Sha256>(), b"Hello#Sensitive1")
        .expect("encrypt");
    let encrypted_b64 = BASE64_STANDARD.encode(encrypted);
    let decrypted = decrypt_base64_with_private_key(private_pem.as_str(), encrypted_b64.as_str())
        .expect("decrypt");
    assert_eq!(decrypted, "Hello#Sensitive1");
}

#[test]
fn forge_js_rsa_oaep_sha256_compat() {
    let (public_pem, private_pem) = generate_key_pair_pem().expect("generate keypair");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().expect("mei-lang repo root");
    let script = repo_root.join("scripts/verify-forge-rsa-compat.mjs");
    assert!(
        script.is_file(),
        "missing forge compat script: {}",
        script.display()
    );
    let public_file = std::env::temp_dir().join("mei-auth-forge-public.pem");
    fs::write(&public_file, public_pem.as_str()).expect("write public pem");
    let output = std::process::Command::new("node")
        .arg(script.as_os_str())
        .arg(&public_file)
        .arg("Hello#Sensitive1")
        .output()
        .expect("run forge compat script");
    let _ = fs::remove_file(public_file);
    assert!(
        output.status.success(),
        "forge compat script failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let encrypted_b64 = String::from_utf8(output.stdout).expect("utf8 stdout");
    let decrypted = decrypt_base64_with_private_key(private_pem.as_str(), encrypted_b64.trim())
        .expect("decrypt");
    assert_eq!(decrypted, "Hello#Sensitive1");
}

#[test]
fn prepare_auth_fails_without_users_when_required() {
    let source_root = temp_source_root("prepare-no-users");
    let err = prepare_auth_for_serve(source_root.as_path(), AuthEnforcement::Required)
        .expect_err("should fail without users");
    let message = err.to_string();
    assert!(message.contains("已启用 --auth"));
    assert!(message.contains("auth.users 为空"));
    assert!(message.contains("host auth bootstrap-users"));
    assert!(message.contains("mei serve --auth"));
}

#[test]
fn format_auth_not_ready_lists_missing_keys() {
    let source_root = temp_source_root("prepare-missing-keys");
    let bundle = load_workspace_auth_bundle(source_root.as_path());
    let runtime = load_auth_runtime(source_root.as_path()).expect("runtime");
    let message = format_auth_not_ready_message(source_root.as_path(), &bundle, &runtime);
    assert!(message.contains("缺少密钥"));
    assert!(message.contains("host auth ensure-keys"));
}

#[test]
fn auth_mutation_appends_workspace_journal_entry() {
    let source_root = temp_source_root("auth-journal");
    ensure_workspace_auth_base(source_root.as_path()).expect("ensure auth base");
    let hash = hash_password("GuestPwd1!safe").expect("hash");
    upsert_workspace_user(
        source_root.as_path(),
        "guest-journal",
        "访客",
        AuthRole::Guest,
        hash.as_str(),
        &[],
        &[],
        &BTreeMap::new(),
    )
    .expect("upsert");
    let journal = AuthJournal::load(source_root.as_path());
    assert!(journal.revision >= 1);
    assert!(!journal.entries.is_empty());
}

#[tokio::test]
async fn disabled_auth_session_api_returns_not_found() {
    let source_root = temp_source_root("disabled-auth-api");
    let state = make_state(source_root.clone(), AuthEnforcement::Disabled);
    let app = Router::new()
        .route(
            "/api/auth/session",
            get(crate::http::auth_api::auth_session),
        )
        .with_state(state);
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/api/auth/session")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[test]
fn prepare_auth_skips_when_disabled() {
    let source_root = temp_source_root("prepare-disabled");
    prepare_auth_for_serve(source_root.as_path(), AuthEnforcement::Disabled)
        .expect("disabled should pass");
}

#[tokio::test]
async fn required_auth_blocks_even_when_users_not_configured() {
    let source_root = temp_source_root("required-no-users");
    ensure_workspace_auth_base(source_root.as_path()).expect("ensure base");
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route("/", get(|| async { "home" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
}

