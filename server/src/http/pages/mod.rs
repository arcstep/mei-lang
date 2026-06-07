//! 管理端 / 访问端 HTML 页面、数据集查询 API 与静态资源合并下发。

mod app;
mod app_render;
mod assets;
mod components;
pub mod dataset_api;
mod menus;
pub mod metric_api;
mod scene_qualified;
mod static_serve;
mod util;

pub use app::{app_page, index};
pub use assets::{app_asset, app_bundle, workspace_app_asset};
pub use components::component_asset;
pub use dataset_api::{dataset_query_api, dataset_recompute_api};
pub use metric_api::dataset_metric_api;

#[cfg(test)]
mod tests;
