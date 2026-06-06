//! 外部 Martin 瓦片服务地址（Docker / 本机独立进程），由 `.env` 或环境变量配置。
//! 应用级 `ops.basemaps` 可覆盖默认 TileJSON 接缝（`basemap_ref` 真源）。

use std::path::Path;

use mei_lang_kernel::{load_mei_config_for_app, OpsBasemapEntry};

#[derive(Clone, Debug)]
pub struct GisTilesConfig {
    /// 例如 `http://127.0.0.1:8080`（无末尾斜杠）
    pub base_url: String,
    /// Martin TileJSON 路径，例如 `/shapingba-z10-16`
    pub json_path: String,
}

impl GisTilesConfig {
    pub fn resolve() -> Self {
        let base_url = std::env::var("MEI_TILES_BASE_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "http://127.0.0.1:8080".to_string());
        let json_path = std::env::var("MEI_TILES_JSON_PATH")
            .ok()
            .map(|s| {
                let t = s.trim().to_string();
                if t.is_empty() {
                    "/shapingba-z10-16".to_string()
                } else if t.starts_with('/') {
                    t
                } else {
                    format!("/{t}")
                }
            })
            .unwrap_or_else(|| "/shapingba-z10-16".to_string());
        Self {
            base_url,
            json_path,
        }
    }

    pub fn resolve_for_app(
        app_root: &Path,
        source_root: Option<&Path>,
        basemap_id: Option<&str>,
    ) -> Self {
        let mut cfg = Self::resolve();
        let mei = load_mei_config_for_app(app_root, source_root);
        let id = basemap_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                mei.ops
                    .params
                    .get("default_basemap")
                    .and_then(|value| value.as_str())
            })
            .unwrap_or("default");
        if let Some(entry) = mei.ops.basemaps.get(id) {
            cfg.apply_basemap_entry(entry);
        } else if let Some((_, entry)) = mei.ops.basemaps.iter().next() {
            cfg.apply_basemap_entry(entry);
        }
        cfg
    }

    fn apply_basemap_entry(&mut self, entry: &OpsBasemapEntry) {
        if let Some(base_url) = entry
            .tiles_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            self.base_url = base_url.to_string();
        }
        if let Some(json_path) = entry
            .tilejson_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            self.json_path = if json_path.starts_with('/') {
                json_path.to_string()
            } else {
                format!("/{json_path}")
            };
        }
    }
}
