#[test]
fn sanitize_next_path_rejects_external_urls() {
    use crate::sanitize_next_path;
    assert_eq!(sanitize_next_path(Some("//evil.example")), "/");
    assert_eq!(sanitize_next_path(Some("/apps/app/demo/scene/home")), "/apps/app/demo/scene/home");
}

#[test]
fn guest_scene_allowlist_blocks_unlisted_scene() {
    use crate::{authorize_path, AuthPrincipal, AuthRole};
    use std::collections::{BTreeMap, BTreeSet};

    let principal = AuthPrincipal {
        username: "guest".into(),
        profile: String::new(),
        role: AuthRole::Guest,
        app_allowlist: BTreeSet::from(["demo".to_string()]),
        app_denylist: BTreeSet::new(),
        scene_allowlist: BTreeMap::from([(
            "demo".to_string(),
            BTreeSet::from(["home".to_string()]),
        )]),
        session_exp: 0,
    };
    assert!(authorize_path("/apps/app/demo/scene/home", &principal).is_ok());
    assert!(authorize_path("/apps/app/demo/scene/other", &principal).is_err());
    assert!(authorize_path("/apps/copilot/demo/presentation/intro", &principal).is_ok());
    assert!(authorize_path("/apps/runtime/demo", &principal).is_err());
}

#[test]
fn super_can_access_runtime_and_copilot_routes() {
    use crate::{authorize_path, AuthPrincipal, AuthRole};
    use std::collections::{BTreeMap, BTreeSet};

    let principal = AuthPrincipal {
        username: "super".into(),
        profile: String::new(),
        role: AuthRole::Super,
        app_allowlist: BTreeSet::new(),
        app_denylist: BTreeSet::new(),
        scene_allowlist: BTreeMap::new(),
        session_exp: 0,
    };
    assert!(authorize_path("/apps/runtime/demo", &principal).is_ok());
    assert!(authorize_path("/apps/copilot/demo/presentation/intro", &principal).is_ok());
}
