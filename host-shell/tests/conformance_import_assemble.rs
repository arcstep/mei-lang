//! Platform conformance: host-shell import/assemble smoke on in-repo fixture.

use mei_host_graph::assemble_scope_from_registry;
use mei_test_support::{ensure_imported, APP_STRUCTURE};

#[test]
fn conformance_import_assemble_home() {
    let workspace = ensure_imported(APP_STRUCTURE);
    let outcome = assemble_scope_from_registry(workspace.as_path(), APP_STRUCTURE, "home")
        .expect("assemble")
        .expect("home outcome");
    assert!(
        outcome.compiled.scene_contract.is_some(),
        "home assemble should produce scene_contract"
    );
}
