use axum::response::{IntoResponse, Redirect, Response};
use mei_lang_kernel::{
    is_stock_catalog_app_for_root, stock_catalog_app_id, BuildNodeId, BuildNodeKind,
};

use crate::AppState;

use super::super::query::{build_query_suffix_with_options, AppQuery, BuildQuerySuffixOptions};

pub(super) fn try_catalog_redirect(
    state: &AppState,
    app_id: &str,
    query: &AppQuery,
) -> Option<Response> {
    if is_stock_catalog_app_for_root(state.source_root.as_path(), app_id) {
        let pack = query
            .pack
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if pack.is_none() {
            if let Ok(discovery) =
                mei_lang_kernel::discover_stock_catalog_packs(state.source_root.as_path())
            {
                let facet = query
                    .catalog
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("components");
                let first_pack = if facet == "templates" {
                    discovery.template_packs.first()
                } else {
                    discovery.component_packs.first()
                };
                if let Some(default_pack) = first_pack {
                    let mut redirected = query.clone();
                    redirected.catalog = Some(facet.to_string());
                    redirected.pack = Some(default_pack.clone());
                    return Some(
                        Redirect::temporary(&format!(
                            "/apps/{app_id}/layout{}",
                            build_query_suffix_with_options(
                                &redirected,
                                BuildQuerySuffixOptions {
                                    include_node: true,
                                    include_scope: true,
                                    include_focus: true,
                                },
                            )
                        ))
                        .into_response(),
                    );
                }
            }
        }
    }
    if let Some(node) = query.node.as_deref().and_then(BuildNodeId::parse) {
        if matches!(node.kind, BuildNodeKind::Component | BuildNodeKind::Template)
            && !is_stock_catalog_app_for_root(state.source_root.as_path(), app_id)
        {
            let catalog_id = stock_catalog_app_id(state.source_root.as_path());
            let mut redirected = query.clone();
            redirected.node = Some(node.encode());
            if matches!(node.kind, BuildNodeKind::Component) {
                redirected.catalog = Some("components".to_string());
                if let Ok(discovery) =
                    mei_lang_kernel::discover_stock_catalog_packs(state.source_root.as_path())
                {
                    let pack = discovery
                        .component_packs
                        .iter()
                        .find(|pack| {
                            node.key.starts_with(pack.as_str()) || node.key.contains(pack.as_str())
                        })
                        .or_else(|| discovery.component_packs.first());
                    if let Some(pack) = pack {
                        redirected.pack = Some(pack.clone());
                    }
                }
            } else if matches!(node.kind, BuildNodeKind::Template) {
                redirected.catalog = Some("templates".to_string());
                if let Ok(discovery) =
                    mei_lang_kernel::discover_stock_catalog_packs(state.source_root.as_path())
                {
                    let top = node.key.split('/').next().unwrap_or("");
                    let pack = discovery
                        .template_packs
                        .iter()
                        .find(|pack| *pack == top)
                        .or_else(|| discovery.template_packs.first());
                    if let Some(pack) = pack {
                        redirected.pack = Some(pack.clone());
                    }
                }
            }
            return Some(
                Redirect::temporary(&format!(
                    "/apps/{catalog_id}/layout{}",
                    build_query_suffix_with_options(
                        &redirected,
                        BuildQuerySuffixOptions {
                            include_node: true,
                            include_scope: true,
                            include_focus: true,
                        },
                    )
                ))
                .into_response(),
            );
        }
    }
    None
}
