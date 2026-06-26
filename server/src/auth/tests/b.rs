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

#[tokio::test]
async fn disabled_auth_allows_anonymous_access() {
    let source_root = temp_source_root("auth-disabled");
    let state = make_state(source_root.clone(), AuthEnforcement::Disabled);
    let app = Router::new()
        .route("/apps/build/demo", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/apps/build/demo")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn middleware_redirects_unauthenticated_pages_with_next() {
    let source_root = temp_source_root("redirect");
    bootstrap_guest_user(source_root.as_path(), &["demo"]);
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route("/apps/build/demo", get(|| async { "ok" }))
        .route("/api/host/ready", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let req = Request::builder()
        .uri("/apps/build/demo?tab=preview")
        .body(Body::empty())
        .expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    assert!(
        location.contains("/login?next=/apps/build/demo%3Ftab%3Dpreview"),
        "unexpected location: {location}"
    );

    let api_req = Request::builder()
        .uri("/api/ops/config/demo")
        .body(Body::empty())
        .expect("api request");
    let api_resp = app.clone().oneshot(api_req).await.expect("api response");
    assert_eq!(api_resp.status(), StatusCode::UNAUTHORIZED);

    let ready_req = Request::builder()
        .uri("/api/host/ready")
        .body(Body::empty())
        .expect("ready request");
    let ready_resp = app.oneshot(ready_req).await.expect("ready response");
    assert_eq!(ready_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn middleware_allows_guest_runtime_assets_and_access_agent() {
    let source_root = temp_source_root("guest-runtime");
    bootstrap_guest_user(source_root.as_path(), &[]);
    let token = token_for(source_root.as_path(), "guest01", "GuestPwd1!safe");
    let runtime = load_auth_runtime(source_root.as_path()).expect("runtime");
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route(
            "/workspace-components/chart/echarts/column.js",
            get(|| async { "ok" }),
        )
        .route("/api/agent/model/probe", get(|| async { "ok" }))
        .route("/api/agent/session", get(|| async { "ok" }))
        .route("/api/agent/start", post(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);
    let cookie = format!("{}={}", runtime.cookie_name, token);

    for path in [
        "/workspace-components/chart/echarts/column.js",
        "/api/agent/model/probe",
        "/api/agent/session",
    ] {
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::COOKIE, cookie.as_str())
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(resp.status(), StatusCode::OK, "path={path}");
    }

    let resp_start = app
        .oneshot(
            Request::builder()
                .uri("/api/agent/start")
                .header(header::COOKIE, cookie.as_str())
                .body(Body::empty())
                .expect("start request"),
        )
        .await
        .expect("start response");
    assert_eq!(resp_start.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn middleware_blocks_guest_authoring_and_unauthorized_app() {
    let source_root = temp_source_root("guest-deny");
    bootstrap_guest_user(source_root.as_path(), &["demo"]);
    let token = token_for(source_root.as_path(), "guest01", "GuestPwd1!safe");
    let runtime = load_auth_runtime(source_root.as_path()).expect("runtime");
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route("/apps/build/demo", get(|| async { "ok" }))
        .route("/apps/app/blocked/scene/home", get(|| async { "ok" }))
        .route("/api/ops/config/demo", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let cookie = format!("{}={}", runtime.cookie_name, token);
    let req_authoring = Request::builder()
        .uri("/apps/build/demo")
        .header(header::COOKIE, cookie.as_str())
        .body(Body::empty())
        .expect("authoring request");
    let resp_authoring = app
        .clone()
        .oneshot(req_authoring)
        .await
        .expect("authoring response");
    assert_eq!(resp_authoring.status(), StatusCode::FORBIDDEN);

    let req_blocked_app = Request::builder()
        .uri("/apps/app/blocked/scene/home")
        .header(header::COOKIE, cookie.as_str())
        .body(Body::empty())
        .expect("blocked app request");
    let resp_blocked_app = app
        .clone()
        .oneshot(req_blocked_app)
        .await
        .expect("blocked app response");
    assert_eq!(resp_blocked_app.status(), StatusCode::FORBIDDEN);

    let req_ops_api = Request::builder()
        .uri("/api/ops/config/demo")
        .header(header::COOKIE, cookie.as_str())
        .body(Body::empty())
        .expect("ops request");
    let resp_ops_api = app.oneshot(req_ops_api).await.expect("ops response");
    assert_eq!(resp_ops_api.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn middleware_allows_admin_authoring_routes() {
    let source_root = temp_source_root("admin-allow");
    bootstrap_admin_user(source_root.as_path());
    let token = token_for(source_root.as_path(), "admin01", "AdminPwd1!safe");
    let runtime = load_auth_runtime(source_root.as_path()).expect("runtime");
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route("/apps/build/demo", get(|| async { "ok" }))
        .route("/apps/config/demo", get(|| async { "ok" }))
        .route("/api/ops/config/demo", get(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);
    let cookie = format!("{}={}", runtime.cookie_name, token);

    let build_req = Request::builder()
        .uri("/apps/build/demo")
        .header(header::COOKIE, cookie.as_str())
        .body(Body::empty())
        .expect("build req");
    let build_resp = app.clone().oneshot(build_req).await.expect("build resp");
    assert_eq!(build_resp.status(), StatusCode::FORBIDDEN);

    let config_req = Request::builder()
        .uri("/apps/config/demo")
        .header(header::COOKIE, cookie.as_str())
        .body(Body::empty())
        .expect("config req");
    let config_resp = app.clone().oneshot(config_req).await.expect("config resp");
    assert_eq!(config_resp.status(), StatusCode::OK);

    let api_req = Request::builder()
        .uri("/api/ops/config/demo")
        .header(header::COOKIE, cookie.as_str())
        .body(Body::empty())
        .expect("api req");
    let api_resp = app.oneshot(api_req).await.expect("api resp");
    assert_eq!(api_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn middleware_allows_unauthenticated_gis_tile_proxy() {
    let source_root = temp_source_root("gis-public");
    bootstrap_admin_user(source_root.as_path());
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route("/gis/*path", get(|| async { "tilejson" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let req = Request::builder()
        .uri("/gis/shapingba-z10-16")
        .body(Body::empty())
        .expect("gis request");
    let resp = app.oneshot(req).await.expect("gis response");
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn middleware_allows_unauthenticated_maplibre_vendor_assets() {
    let source_root = temp_source_root("maplibre-public");
    bootstrap_guest_user(source_root.as_path(), &["demo"]);
    let token = token_for(source_root.as_path(), "guest01", "GuestPwd1!safe");
    let runtime = load_auth_runtime(source_root.as_path()).expect("runtime");
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route(
            "/workspace-components/vendor/maplibre/fonts/*path",
            get(|| async { "glyphs" }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    let unauth_req = Request::builder()
        .uri("/workspace-components/vendor/maplibre/fonts/Open%20Sans%20Regular,Arial%20Unicode%20MS%20Regular/0-255.pbf")
        .body(Body::empty())
        .expect("glyph request");
    let unauth_resp = app
        .clone()
        .oneshot(unauth_req)
        .await
        .expect("glyph response");
    assert_eq!(unauth_resp.status(), StatusCode::OK);

    let cookie = format!("{}={}", runtime.cookie_name, token);
    let guest_req = Request::builder()
        .uri("/workspace-components/vendor/maplibre/fonts/Open%20Sans%20Regular,Arial%20Unicode%20MS%20Regular/0-255.pbf")
        .header(header::COOKIE, cookie.as_str())
        .body(Body::empty())
        .expect("guest glyph request");
    let guest_resp = app.oneshot(guest_req).await.expect("guest glyph response");
    assert_eq!(guest_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn auth_refresh_extends_session_exp() {
    use axum::body::to_bytes;

    let source_root = temp_source_root("auth-refresh");
    bootstrap_guest_user(source_root.as_path(), &[]);
    let runtime = load_auth_runtime(source_root.as_path()).expect("runtime");
    let old_token = token_for(source_root.as_path(), "guest01", "GuestPwd1!safe");
    let old_claims = runtime
        .decode_jwt(old_token.as_str())
        .expect("decode old token");
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route(
            "/api/auth/refresh",
            post(crate::http::auth_api::auth_refresh),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);
    let cookie = format!("{}={}", runtime.cookie_name, old_token);
    std::thread::sleep(std::time::Duration::from_secs(1));
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/refresh")
                .header(header::COOKIE, cookie.as_str())
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get(header::SET_COOKIE).is_some());
    let body = to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("response body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json body");
    let new_exp = json
        .get("expiresAt")
        .and_then(serde_json::Value::as_u64)
        .expect("expiresAt");
    assert!(new_exp >= old_claims.exp as u64);
    assert_eq!(
        json.get("ok").and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[tokio::test]
async fn auth_refresh_without_token_returns_401() {
    let source_root = temp_source_root("auth-refresh-unauth");
    bootstrap_guest_user(source_root.as_path(), &[]);
    let state = make_state(source_root.clone(), AuthEnforcement::Required);
    let app = Router::new()
        .route(
            "/api/auth/refresh",
            post(crate::http::auth_api::auth_refresh),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/refresh")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disabled_auth_refresh_returns_not_found() {
    let source_root = temp_source_root("disabled-auth-refresh");
    let state = make_state(source_root.clone(), AuthEnforcement::Disabled);
    let app = Router::new()
        .route(
            "/api/auth/refresh",
            post(crate::http::auth_api::auth_refresh),
        )
        .with_state(state);
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/refresh")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
