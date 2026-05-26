//! 外部 Martin 瓦片服务地址（Docker / 本机独立进程），由 `.env` 或环境变量配置。

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

    pub fn tilejson_url(&self) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), self.json_path)
    }
}
