use mei_lang_kernel::ProvenanceAnchor;

use super::append::append_suggested_tasks;

#[test]
fn suggested_tasks_lock_node_contains_symbol() {
    let mut md = String::new();
    append_suggested_tasks(
        &mut md,
        "lock_node",
        &ProvenanceAnchor {
            file: "metrics.world.mei".to_string(),
            symbol_id: "total".to_string(),
            symbol_kind: "metric".to_string(),
        },
        &None,
    );
    assert!(md.contains("total"));
}
