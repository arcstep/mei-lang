//! 编译期 config ref 解码与解析（theme / source / basemap 等）。

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mei_config::{MeiConfig, OpsBasemapEntry, OpsSourceEntry};
use crate::model::{Diagnostic, Severity, SourceDecl};

pub const CONFIG_REF_SOURCE_KIND: &str = "__config_ref";
pub const THEME_REF_PREFIX: &str = "@theme:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigRefKind {
    Theme,
    Source,
    DatasetSource,
    Resource,
    Basemap,
    Mapspec,
    OpsParam,
}

impl ConfigRefKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Source => "source",
            Self::DatasetSource => "dataset_source",
            Self::Resource => "resource",
            Self::Basemap => "basemap",
            Self::Mapspec => "mapspec",
            Self::OpsParam => "ops_param",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug.trim().to_ascii_lowercase().as_str() {
            "theme" | "theme_ref" => Some(Self::Theme),
            "source" | "source_ref" => Some(Self::Source),
            "dataset_source" | "dataset_source_ref" => Some(Self::DatasetSource),
            "resource" | "resource_ref" => Some(Self::Resource),
            "basemap" | "basemap_ref" => Some(Self::Basemap),
            "mapspec" | "mapspec_ref" => Some(Self::Mapspec),
            "ops_param" | "ops_param_ref" => Some(Self::OpsParam),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigRefExpr {
    pub kind: ConfigRefKind,
    pub id: String,
}

pub fn decode_config_ref_value(value: &Value) -> Option<ConfigRefExpr> {
    let map = value.as_object()?;
    let marker = map.get("__config_ref").and_then(Value::as_str)?;
    let id = map.get("id").and_then(Value::as_str)?.trim();
    if id.is_empty() {
        return None;
    }
    let kind = ConfigRefKind::from_slug(marker)?;
    Some(ConfigRefExpr {
        kind,
        id: id.to_string(),
    })
}

pub fn decode_theme_ref_token(token: &str) -> Option<String> {
    let trimmed = token.trim();
    trimmed
        .strip_prefix(THEME_REF_PREFIX)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
}

pub fn theme_ref_token(id: &str) -> String {
    format!("{THEME_REF_PREFIX}{}", id.trim())
}

pub fn config_ref_to_json(kind: ConfigRefKind, id: &str) -> Value {
    serde_json::json!({
        "__config_ref": kind.as_str(),
        "id": id.trim(),
    })
}

pub fn source_decl_from_value(value: Value) -> Result<SourceDecl, String> {
    if let Some(expr) = decode_config_ref_value(&value) {
        return Ok(SourceDecl {
            kind: CONFIG_REF_SOURCE_KIND.to_string(),
            path: format!("{}:{}", expr.kind.as_str(), expr.id),
            sheet: None,
            header_row: None,
            preview_rows: None,
            page_size: None,
            max_page_size: None,
            table: None,
            query: None,
            connection: None,
            content: None,
        });
    }
    serde_json::from_value(value).map_err(|error| error.to_string())
}

pub fn is_config_ref_source(source: &SourceDecl) -> bool {
    source.kind == CONFIG_REF_SOURCE_KIND
}

pub fn parse_config_ref_path(path: &str) -> Option<ConfigRefExpr> {
    let (kind_slug, id) = path.split_once(':')?;
    let kind = ConfigRefKind::from_slug(kind_slug)?;
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    Some(ConfigRefExpr {
        kind,
        id: id.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct ConfigRefResolver<'a> {
    config: &'a MeiConfig,
}

impl<'a> ConfigRefResolver<'a> {
    pub fn new(config: &'a MeiConfig) -> Self {
        Self { config }
    }

    pub fn resolve_source_entry(&self, id: &str) -> Option<&OpsSourceEntry> {
        self.config.ops.sources.get(id)
    }

    pub fn resolve_basemap_entry(&self, id: &str) -> Option<&OpsBasemapEntry> {
        self.config.ops.basemaps.get(id)
    }

    pub fn resolve_theme_value(&self, id: &str) -> Option<&Value> {
        self.config.ops.themes.get(id)
    }

    pub fn resolve_ops_param(&self, id: &str) -> Option<&Value> {
        self.config.ops.params.get(id)
    }

    pub fn resolve_source_decl(
        &self,
        expr: &ConfigRefExpr,
        target_file: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<SourceDecl> {
        let id = expr.id.as_str();
        let entry = match expr.kind {
            ConfigRefKind::Source | ConfigRefKind::DatasetSource => self.resolve_source_entry(id),
            _ => None,
        };
        let Some(entry) = entry else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_config_ref".to_string(),
                message: format!(
                    "config ref `{}` id `{}` not found in ops registry",
                    expr.kind.as_str(),
                    id
                ),
                source_path: Some(target_file.to_string()),
            });
            return None;
        };
        Some(ops_source_entry_to_decl(entry))
    }

    pub fn resolve_source_decl_from_source(
        &self,
        source: &SourceDecl,
        target_file: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> SourceDecl {
        if !is_config_ref_source(source) {
            return source.clone();
        }
        let Some(expr) = parse_config_ref_path(source.path.as_str()) else {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "invalid_config_ref".to_string(),
                message: format!(
                    "invalid config ref source path `{}`",
                    source.path
                ),
                source_path: Some(target_file.to_string()),
            });
            return source.clone();
        };
        self.resolve_source_decl(&expr, target_file, diagnostics)
            .unwrap_or_else(|| source.clone())
    }

    pub fn resolve_theme_token(&self, token: &str) -> Option<Value> {
        let id = decode_theme_ref_token(token)?;
        let theme = self.resolve_theme_value(id.as_str())?;
        Some(theme.clone())
    }

    pub fn validate_theme_token(
        &self,
        token: &str,
        target_file: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(id) = decode_theme_ref_token(token) else {
            return;
        };
        if self.resolve_theme_value(id.as_str()).is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_config_ref".to_string(),
                message: format!("theme_ref id `{id}` not found in ops.themes"),
                source_path: Some(target_file.to_string()),
            });
        }
    }

    pub fn validate_basemap_ref(
        &self,
        id: &str,
        target_file: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if self.resolve_basemap_entry(id).is_none() {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: "missing_config_ref".to_string(),
                message: format!("basemap_ref id `{id}` not found in ops.basemaps"),
                source_path: Some(target_file.to_string()),
            });
        }
    }
}

pub fn ops_source_entry_to_decl(entry: &OpsSourceEntry) -> SourceDecl {
    SourceDecl {
        kind: entry.kind.clone(),
        path: entry.path.clone(),
        sheet: entry.sheet.clone(),
        header_row: entry.header_row,
        preview_rows: entry.preview_rows,
        page_size: entry.page_size,
        max_page_size: entry.max_page_size,
        table: entry.table.clone(),
        query: entry.query.clone(),
        connection: entry.connection.clone(),
        content: None,
    }
}

pub fn walk_value_for_config_refs(
    value: &Value,
    target_file: &str,
    resolver: &ConfigRefResolver<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match value {
        Value::Object(map) => {
            if let Some(expr) = decode_config_ref_value(value) {
                match expr.kind {
                    ConfigRefKind::Basemap | ConfigRefKind::Mapspec => {
                        resolver.validate_basemap_ref(expr.id.as_str(), target_file, diagnostics);
                    }
                    ConfigRefKind::Theme => {
                        if resolver.resolve_theme_value(expr.id.as_str()).is_none() {
                            diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                code: "missing_config_ref".to_string(),
                                message: format!(
                                    "theme_ref id `{}` not found in ops.themes",
                                    expr.id
                                ),
                                source_path: Some(target_file.to_string()),
                            });
                        }
                    }
                    ConfigRefKind::OpsParam => {
                        if resolver.resolve_ops_param(expr.id.as_str()).is_none() {
                            diagnostics.push(Diagnostic {
                                severity: Severity::Error,
                                code: "missing_config_ref".to_string(),
                                message: format!(
                                    "ops_param_ref id `{}` not found in ops.params",
                                    expr.id
                                ),
                                source_path: Some(target_file.to_string()),
                            });
                        }
                    }
                    ConfigRefKind::Source | ConfigRefKind::DatasetSource => {
                        let _ = resolver.resolve_source_decl(&expr, target_file, diagnostics);
                    }
                    ConfigRefKind::Resource => {}
                }
                return;
            }
            for entry in map.values() {
                walk_value_for_config_refs(entry, target_file, resolver, diagnostics);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_value_for_config_refs(item, target_file, resolver, diagnostics);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_source_ref_value() {
        let value = serde_json::json!({"__config_ref": "source", "id": "main"});
        let expr = decode_config_ref_value(&value).expect("expr");
        assert_eq!(expr.kind, ConfigRefKind::Source);
        assert_eq!(expr.id, "main");
    }

    #[test]
    fn theme_ref_token_roundtrip() {
        assert_eq!(
            decode_theme_ref_token("@theme:cockpit").as_deref(),
            Some("cockpit")
        );
    }
}
