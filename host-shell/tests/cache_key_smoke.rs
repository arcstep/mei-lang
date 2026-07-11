//! Cache key smoke: semantic builders roundtrip.

use mei_host_graph::{build_page_render_view_axes, build_semantic_cache_core};

#[test]
fn semantic_cache_core_builder_roundtrip() {
    let core = build_semantic_cache_core(
        "demo",
        "home",
        None,
        "reg-v1",
        "client-v1",
        "data-gen-1",
        "compile-epoch-1",
    );
    let signature = mei_host_graph::semantic_cache_core_signature(&core).expect("signature");
    assert!(signature.contains("demo"));
    assert!(signature.contains("home"));
    assert!(signature.contains("reg-v1"));
}

#[test]
fn page_render_view_axes_include_route_and_projection() {
    let view = build_page_render_view_axes("app", "eval", "live_full", Some(42), None);
    let signature = mei_host_graph::page_render_view_signature(&view).expect("signature");
    assert!(signature.contains("live_full"));
    assert!(signature.contains("\"route_mode\":\"app\""));
}

#[test]
fn build_view_axes_use_build_route_mode() {
    let view = build_page_render_view_axes(
        "build",
        "static",
        "plane_region_section",
        None,
        Some("ops-1".to_string()),
    );
    let signature = mei_host_graph::page_render_view_signature(&view).expect("signature");
    assert!(signature.contains("\"route_mode\":\"build\""));
    assert!(signature.contains("plane_region_section"));
    assert!(signature.contains("ops-1"));
}
