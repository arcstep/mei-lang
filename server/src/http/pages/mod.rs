//! 管理端 / 访问端 HTML 页面、数据集查询 API 与静态资源合并下发。

mod app;
mod app_render;
mod assets;
mod components;
pub mod dataset_api;
mod gis_proxy;
mod host_hub;
mod menus;
pub mod metric_api;
mod scene_qualified;
mod static_serve;
mod util;

pub(crate) use app::clear_page_render_cache;
pub(crate) use app_render::{prepare_landing_artifacts_for_serve, probe_landing_readiness};
pub use app::{app_page, index, AppQuery};
pub use host_hub::host_hub_page;
pub use assets::{app_asset, app_bundle, workspace_app_asset};
pub use components::component_asset;
pub use dataset_api::{dataset_query_api, dataset_recompute_api};
pub use gis_proxy::gis_proxy;
pub use metric_api::dataset_metric_api;
pub(crate) use static_serve::content_type_for_path;

#[cfg(test)]
mod tests;
