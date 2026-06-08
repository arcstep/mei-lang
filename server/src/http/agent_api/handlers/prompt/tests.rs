use axum::{
    body::to_bytes,
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;

use crate::{http, test_support};

#[tokio::test]
async fn context_preview_has_scope_digest_resource_tools_and_boundary() {
    let state = test_support::test_app_state().expect("app state");
    let app = http::router().with_state(state);
    let uri = "/api/agent/context/preview?app_id=examples%2Fcore%2F01-single-file-doc&mode=ask&resourceVisibility=allow_direct_refs";
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v
        .get("scope_digest")
        .and_then(|s| s.as_str())
        .is_some_and(|s| !s.is_empty()));
    let names = v["native_tool_names"].as_array().expect("names");
    let set: std::collections::HashSet<_> = names.iter().filter_map(|x| x.as_str()).collect();
    assert!(set.contains("resource_list"));
    assert!(set.contains("resource_get"));
    assert!(set.contains("resource_runtime_peek"));
    let b = v.get("scope_boundary").expect("boundary");
    assert_eq!(b["binding_scope"], "scene");
    assert_eq!(b["resource_visibility"], "allow_direct_refs");
    assert_eq!(b["edit_scope"], "read_only");
    assert_eq!(
        v["host_contract"]["protocol_schema"],
        "mei-host-runtime-protocol-v1"
    );
    assert!(
        v["runtime_capabilities"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "runtime_capabilities should be non-empty"
    );
    let inv = v.get("resource_inventory").and_then(|x| x.as_object());
    assert!(inv.is_some(), "resource_inventory object");
    let items = inv.unwrap().get("items").and_then(|x| x.as_array());
    assert!(items.is_some());
    let items = items.unwrap();
    if let Some(first) = items.first().and_then(|x| x.as_object()) {
        assert!(
            first.contains_key("reach_tier"),
            "items should include reach_tier"
        );
    }
}

#[tokio::test]
async fn context_preview_scene_id_matches_query() {
    let state = test_support::test_app_state().expect("app state");
    let app = http::router().with_state(state);
    let uri = "/api/agent/context/preview?app_id=examples%2Fds%2F01-dataset-baseline&scene_id=home&target_file=main.mei&mode=ask&resourceVisibility=allow_direct_refs";
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["scene_id"], "home");
    assert_eq!(v["target_file"], "main.mei");
}

#[tokio::test]
async fn context_preview_invalid_app_id_still_ok_with_empty_inventory() {
    let state = test_support::test_app_state().expect("app state");
    let app = http::router().with_state(state);
    let uri = "/api/agent/context/preview?app_id=___not_an_app___&mode=ask&resourceVisibility=allow_direct_refs";
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("resource_inventory").is_none() || v["resource_inventory"].is_null());
}

#[tokio::test]
async fn context_preview_sets_preview_error_when_world_snapshot_fails() {
    let state = test_support::test_app_state().expect("app state");
    let app = http::router().with_state(state);
    let uri = "/api/agent/context/preview?app_id=examples%2Fcore%2F_invalid%2F07-app-missing-scene&target_file=main.mei&mode=ask&resourceVisibility=allow_direct_refs";
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let pe = v
        .get("preview_error")
        .and_then(|x| x.as_str())
        .unwrap_or("");
    assert!(
        !pe.is_empty(),
        "preview_error should surface degraded snapshot reason"
    );
    assert!(v.get("resource_inventory").is_none() || v["resource_inventory"].is_null());
}

#[tokio::test]
async fn context_preview_accepts_app_route_mode_alias_as_access() {
    let state = test_support::test_app_state().expect("app state");
    let app = http::router().with_state(state);
    let uri = "/api/agent/context/preview?app_id=examples%2Fds%2F01-dataset-baseline&scene_id=home&target_file=main.mei&mode=ask&route_mode=app";
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        v["profile_summary"]
            .as_str()
            .unwrap_or_default()
            .contains("route=access"),
        "profile summary should normalize route_mode=app as access"
    );
}

#[tokio::test]
async fn context_preview_scope_digest_changes_with_browser_context() {
    let state = test_support::test_app_state().expect("app state");
    let app = http::router().with_state(state);
    let uri1 = "/api/agent/context/preview?app_id=examples%2Fds%2F01-dataset-baseline&scene_id=home&target_file=main.mei&mode=ask&route_mode=access&browser_context=%7B%22schema%22%3A%22access_browser_context_v1%22%2C%22active_query_state_ids%22%3A%5B%22q1%22%5D%7D";
    let req1 = Request::builder().uri(uri1).body(Body::empty()).unwrap();
    let response1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = to_bytes(response1.into_body(), usize::MAX).await.unwrap();
    let v1: serde_json::Value = serde_json::from_slice(&body1).unwrap();
    let digest1 = v1["scope_digest"].as_str().unwrap_or_default().to_string();
    assert!(!digest1.is_empty(), "scope_digest should not be empty");

    let uri2 = "/api/agent/context/preview?app_id=examples%2Fds%2F01-dataset-baseline&scene_id=home&target_file=main.mei&mode=ask&route_mode=access&browser_context=%7B%22schema%22%3A%22access_browser_context_v1%22%2C%22active_query_state_ids%22%3A%5B%22q2%22%5D%7D";
    let req2 = Request::builder().uri(uri2).body(Body::empty()).unwrap();
    let response2 = app.oneshot(req2).await.unwrap();
    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = to_bytes(response2.into_body(), usize::MAX).await.unwrap();
    let v2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    let digest2 = v2["scope_digest"].as_str().unwrap_or_default().to_string();
    assert!(!digest2.is_empty(), "scope_digest should not be empty");
    assert_ne!(
        digest1, digest2,
        "scope_digest should change when browser_context changes"
    );
    assert_eq!(
        v2["browser_context_echo"]["active_query_state_ids"][0],
        "q2"
    );
}

#[tokio::test]
async fn context_preview_echoes_host_protocol_and_affects_scope_digest() {
    let state = test_support::test_app_state().expect("app state");
    let app = http::router().with_state(state);
    let uri1 = "/api/agent/context/preview?app_id=examples%2Fds%2F01-dataset-baseline&scene_id=home&target_file=main.mei&mode=ask&route_mode=access&host_protocol=%7B%22schema%22%3A%22mei-host-runtime-protocol-v1%22%2C%22surface%22%3A%22access_host%22%2C%22route_mode%22%3A%22access%22%2C%22mode%22%3A%22ask%22%7D&host_contract_schema=mei-host-runtime-contract-v1";
    let req1 = Request::builder().uri(uri1).body(Body::empty()).unwrap();
    let response1 = app.clone().oneshot(req1).await.unwrap();
    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = to_bytes(response1.into_body(), usize::MAX).await.unwrap();
    let v1: serde_json::Value = serde_json::from_slice(&body1).unwrap();
    let digest1 = v1["scope_digest"].as_str().unwrap_or_default().to_string();
    assert!(!digest1.is_empty(), "scope_digest should not be empty");
    assert_eq!(
        v1["host_protocol_echo"]["schema"],
        "mei-host-runtime-protocol-v1"
    );
    assert_eq!(
        v1["host_contract_schema_echo"],
        "mei-host-runtime-contract-v1"
    );

    let uri2 = "/api/agent/context/preview?app_id=examples%2Fds%2F01-dataset-baseline&scene_id=home&target_file=main.mei&mode=ask&route_mode=access&host_protocol=%7B%22schema%22%3A%22mei-host-runtime-protocol-v1%22%2C%22surface%22%3A%22authoring_host%22%2C%22route_mode%22%3A%22access%22%2C%22mode%22%3A%22ask%22%7D";
    let req2 = Request::builder().uri(uri2).body(Body::empty()).unwrap();
    let response2 = app.oneshot(req2).await.unwrap();
    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = to_bytes(response2.into_body(), usize::MAX).await.unwrap();
    let v2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    let digest2 = v2["scope_digest"].as_str().unwrap_or_default().to_string();
    assert!(!digest2.is_empty(), "scope_digest should not be empty");
    assert_ne!(
        digest1, digest2,
        "scope_digest should change when host_protocol changes"
    );
    assert_eq!(v2["host_protocol_echo"]["surface"], "authoring_host");
}
