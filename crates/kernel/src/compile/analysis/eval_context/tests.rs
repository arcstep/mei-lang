use super::{EvalContext, EvalNodeKind, RuntimeMetricEvalScope};
use crate::model::QueryState;
use serde_json::json;

#[test]
fn eval_context_keys_include_scope_fingerprint() {
    let expr = json!({"__kind":"analysis_expr","type":"count"});
    let mut left = EvalContext::with_scope(RuntimeMetricEvalScope {
        base_dataset_id: "warning_list".to_string(),
        scene_id: "home".to_string(),
        target: "scenes/home.mei".to_string(),
        search: String::new(),
        query_state: QueryState::default(),
        filter_intents: Vec::new(),
        dimension_bindings: Vec::new(),
        filters_fingerprint: "{\"status\":\"待办\"}".to_string(),
        dependency_revision_key: "deps=a".to_string(),
    });
    let mut right = EvalContext::with_scope(RuntimeMetricEvalScope {
        base_dataset_id: "warning_list".to_string(),
        scene_id: "home".to_string(),
        target: "scenes/home.mei".to_string(),
        search: String::new(),
        query_state: QueryState::default(),
        filter_intents: Vec::new(),
        dimension_bindings: Vec::new(),
        filters_fingerprint: "{\"status\":\"已办\"}".to_string(),
        dependency_revision_key: "deps=a".to_string(),
    });
    left.store_scalar(&expr, &json!(3));
    right.store_scalar(&expr, &json!(4));
    assert_eq!(left.scalar(&expr), Some(json!(3)));
    assert_eq!(right.scalar(&expr), Some(json!(4)));
}

#[test]
fn eval_context_cycle_guard_rejects_reentry() {
    let scope = RuntimeMetricEvalScope {
        base_dataset_id: "warning_list".to_string(),
        scene_id: "home".to_string(),
        target: "scenes/home.mei".to_string(),
        search: String::new(),
        query_state: QueryState::default(),
        filter_intents: Vec::new(),
        dimension_bindings: Vec::new(),
        filters_fingerprint: "{}".to_string(),
        dependency_revision_key: "deps=a".to_string(),
    };
    let mut ctx = EvalContext::with_scope(scope);
    let expr = json!({"__kind":"analysis_expr","type":"count"});
    let key = ctx.rowset_key(&expr).expect("rowset key");
    ctx.begin_eval_node(&key).expect("first begin should pass");
    let err = ctx.begin_eval_node(&key).expect_err("reentry should fail");
    assert!(err.to_string().contains("cyclic_eval_dependency"));
}

#[test]
fn eval_context_canonicalizes_expr_key_order() {
    let scope = RuntimeMetricEvalScope {
        base_dataset_id: "warning_list".to_string(),
        scene_id: "home".to_string(),
        target: "scenes/home.mei".to_string(),
        search: String::new(),
        query_state: QueryState::default(),
        filter_intents: Vec::new(),
        dimension_bindings: Vec::new(),
        filters_fingerprint: "{}".to_string(),
        dependency_revision_key: "deps=a".to_string(),
    };
    let mut ctx = EvalContext::with_scope(scope);
    let left = json!({"__kind":"analysis_expr","type":"count","rowset":{"b":2,"a":1}});
    let right = json!({"type":"count","__kind":"analysis_expr","rowset":{"a":1,"b":2}});
    ctx.store_scalar(&left, &json!(9));
    assert_eq!(ctx.cached_scalar(&right), Some(json!(9)));
}

#[test]
fn request_dag_metrics_track_nested_eval_edges_and_request_hits() {
    let scope = RuntimeMetricEvalScope {
        base_dataset_id: "warning_list".to_string(),
        scene_id: "home".to_string(),
        target: "scenes/home.mei".to_string(),
        search: String::new(),
        query_state: QueryState::default(),
        filter_intents: Vec::new(),
        dimension_bindings: Vec::new(),
        filters_fingerprint: "{\"status\":\"待办\"}".to_string(),
        dependency_revision_key: "deps=a".to_string(),
    };
    let mut ctx = EvalContext::with_scope(scope);
    let parent = json!({"__kind":"analysis_expr","type":"count","name":"parent"});
    let child = json!({"__kind":"analysis_expr","type":"where","name":"child"});
    ctx.store_rowset(&child, &[json!({"id": 1})]);
    ctx.with_eval_node(
        &ctx.scalar_key(&parent).expect("parent scalar key"),
        EvalNodeKind::Scalar,
        |ctx| {
            assert_eq!(ctx.cached_rowset(&child).unwrap_or_default().len(), 1);
            Ok(json!(1))
        },
    )
    .expect("nested eval should succeed");
    let metrics = ctx.request_dag_metrics();
    assert_eq!(metrics.nodes, 2, "parent scalar node and child rowset node");
    assert_eq!(metrics.edges, 1, "parent should depend on child");
    assert_eq!(
        metrics.request_cache_hits, 1,
        "child should hit request cache"
    );
}
