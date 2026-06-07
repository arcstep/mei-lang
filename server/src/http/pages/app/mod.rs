mod compiling_shell;
mod page;
mod query;
mod scene;

pub use page::app_page;

/// 仅用于 `http::pages` 集成测试；运行态页面从 `query` 模块直接引用 `AppQuery`。
#[cfg(test)]
pub(crate) type AppQuery = query::AppQuery;

use std::path::Path;

use axum::{extract::State, response::Redirect, Extension};
use mei_lang_kernel::{discover_apps, resolve_default_scene_from_root, HostSurface, WorkspaceAppMeta};

use crate::{
    auth::AuthPrincipal,
    AppError, AppState,
};

use super::app_render::choose_default_app;
use query::access_canonical_location;

pub(crate) fn index_landing_location(
    source_root: &Path,
    app: &WorkspaceAppMeta,
    principal: Option<&AuthPrincipal>,
    access_only_surface: bool,
) -> String {
    if access_only_surface {
        return format!("/apps/access-only/{}", app.id);
    }
    let mode = match principal {
        None => "build",
        Some(p) if p.can_use_build_surface() => "build",
        Some(_) => "app",
    };
    if mode == "app" {
        let app_root = source_root.join(&app.id);
        let scene = resolve_default_scene_from_root(&app_root)
            .ok()
            .flatten()
            .filter(|scene| !scene.trim().is_empty())
            .unwrap_or_else(|| "home".to_string());
        if let Some(p) = principal {
            if !p.can_access_scene(app.id.as_str(), scene.as_str()) {
                return format!("/apps/app/{}", app.id);
            }
        }
        return access_canonical_location(&app.id, scene.as_str(), None, None);
    }
    format!("/apps/{mode}/{}", app.id)
}

fn filter_apps_for_principal(
    apps: &[WorkspaceAppMeta],
    principal: Option<&AuthPrincipal>,
) -> Vec<WorkspaceAppMeta> {
    apps.iter()
        .filter(|app| {
            principal
                .map(|p| p.can_access_app(app.id.as_str()))
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

pub async fn index(
    State(state): State<AppState>,
    principal: Option<Extension<AuthPrincipal>>,
) -> Result<Redirect, AppError> {
    let principal = principal.map(|Extension(value)| value);
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let filtered = filter_apps_for_principal(&apps, principal.as_ref());
    let first = choose_default_app(&state.source_root, &filtered)
        .or_else(|| filtered.first())
        .or_else(|| choose_default_app(&state.source_root, &apps))
        .or_else(|| apps.first());
    let first = first.ok_or_else(|| {
        AppError::msg(format!(
            "source root has no discoverable apps (need at least one first-level subdirectory under `{}` containing `main.mei`; root-level `main.mei` is ignored)",
            state.source_root.display()
        ))
    })?;
    let access_only_surface = std::env::var("MEI_HOST_SURFACE")
        .ok()
        .map(|value| HostSurface::from_host_surface_flag(&value))
        .is_some_and(|surface| surface == HostSurface::AccessOnlyHost);
    let location = index_landing_location(
        state.source_root.as_path(),
        first,
        principal.as_ref(),
        access_only_surface,
    );
    Ok(Redirect::to(&location))
}

#[cfg(test)]
mod tests {
    use super::{index_landing_location, filter_apps_for_principal};
    use crate::auth::{AuthPrincipal, AuthRole};
    use mei_lang_kernel::WorkspaceAppMeta;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    fn sample_app(id: &str) -> WorkspaceAppMeta {
        WorkspaceAppMeta {
            id: id.to_string(),
            title: id.to_string(),
            root: id.to_string(),
        }
    }

    #[test]
    fn index_landing_uses_build_for_anonymous_and_app_for_admin() {
        let root = Path::new("/tmp/ws");
        let app = sample_app("demo");
        let admin = AuthPrincipal {
            username: "admin".into(),
            profile: String::new(),
            role: AuthRole::Admin,
            app_allowlist: BTreeSet::new(),
            app_denylist: BTreeSet::new(),
            scene_allowlist: BTreeMap::new(),
        };
        assert_eq!(
            index_landing_location(root, &app, None, false),
            "/apps/build/demo"
        );
        assert!(
            index_landing_location(root, &app, Some(&admin), false).starts_with("/apps/app/demo")
        );
    }

    #[test]
    fn filter_apps_respects_guest_allowlist() {
        let apps = vec![sample_app("a"), sample_app("b")];
        let guest = AuthPrincipal {
            username: "g".into(),
            profile: String::new(),
            role: AuthRole::Guest,
            app_allowlist: ["b".to_string()].into_iter().collect(),
            app_denylist: BTreeSet::new(),
            scene_allowlist: BTreeMap::new(),
        };
        let filtered = filter_apps_for_principal(&apps, Some(&guest));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id.as_str(), "b");
    }
}
