#[test]
fn sanitize_next_path_rejects_external_urls() {
    use crate::sanitize_next_path;
    assert_eq!(sanitize_next_path(Some("//evil.example")), "/");
    assert_eq!(
        sanitize_next_path(Some("/apps/app/demo/scene/home")),
        "/apps/app/demo/scene/home"
    );
}

#[test]
fn app_first_path_parses_app_and_stage() {
    use crate::{authorize_path, AuthPrincipal, AuthRole};
    use std::collections::{BTreeMap, BTreeSet};

    let principal = AuthPrincipal {
        username: "guest".into(),
        profile: String::new(),
        role: AuthRole::Guest,
        app_allowlist: BTreeSet::from(["mini-data".to_string()]),
        app_denylist: BTreeSet::new(),
        scene_allowlist: BTreeMap::from([(
            "mini-data".to_string(),
            BTreeSet::from(["home".to_string()]),
        )]),
        session_exp: 0,
    };
    // Canonical Access: `/apps/{app}/{stage}` must not treat app id as mode.
    assert!(authorize_path("/apps/mini-data/home", &principal).is_ok());
    assert!(authorize_path("/apps/mini-data", &principal).is_ok());
    assert!(authorize_path("/apps/mini-data/other", &principal).is_err());
    assert!(authorize_path("/apps/unknown-app/home", &principal).is_err());
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
    assert!(authorize_path("/runtime", &principal).is_ok());
    assert!(authorize_path("/apps/copilot/demo/presentation/intro", &principal).is_ok());
}

#[test]
fn admin_can_access_app_center_but_not_legacy_apps_runtime() {
    use crate::{authorize_path, AuthPrincipal, AuthRole};
    use std::collections::{BTreeMap, BTreeSet};

    let principal = AuthPrincipal {
        username: "admin".into(),
        profile: String::new(),
        role: AuthRole::Admin,
        app_allowlist: BTreeSet::new(),
        app_denylist: BTreeSet::new(),
        scene_allowlist: BTreeMap::new(),
        session_exp: 0,
    };
    assert!(authorize_path("/runtime", &principal).is_ok());
    assert!(authorize_path("/host/runtime", &principal).is_ok());
    assert!(authorize_path("/config", &principal).is_ok());
    assert!(principal.can_access_host_route_mode("runtime"));
    assert!(authorize_path("/apps/runtime/demo", &principal).is_err());
}

#[test]
fn guest_cannot_access_app_center() {
    use crate::{authorize_path, AuthPrincipal, AuthRole};
    use std::collections::{BTreeMap, BTreeSet};

    let principal = AuthPrincipal {
        username: "guest".into(),
        profile: String::new(),
        role: AuthRole::Guest,
        app_allowlist: BTreeSet::new(),
        app_denylist: BTreeSet::new(),
        scene_allowlist: BTreeMap::new(),
        session_exp: 0,
    };
    assert!(authorize_path("/runtime", &principal).is_err());
    assert!(!principal.can_access_host_route_mode("runtime"));
}
