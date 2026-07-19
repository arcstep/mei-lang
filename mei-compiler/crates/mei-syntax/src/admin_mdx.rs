//! Restricted v2 Admin Entry MDX front-end.
//!
//! Identity and routing are deliberately absent: the kernel derives them from
//! `src/admin/{resource_id}/{module_id}.mdx`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::stage_mdx::{
    check_markdown_line, find_frontmatter_end, markdown_from_lines, parse_named_directive_args,
    unquote, validate_id_token, MarkdownForbidden, StageMarkdown,
};

pub const ADMIN_API_VERSION: &str = "mei-admin-resource-v2";
pub const ADMIN_MDX_PARSE: &str = "admin_mdx_parse";
pub const ADMIN_MDX_JSX_FORBIDDEN: &str = "admin_mdx_jsx_forbidden";
pub const ADMIN_MDX_JS_FORBIDDEN: &str = "admin_mdx_js_forbidden";
pub const ADMIN_IDENTITY_REDECLARATION_FORBIDDEN: &str = "admin_identity_redeclaration_forbidden";
pub const ADMIN_FRONTMATTER_FIELD_FORBIDDEN: &str = "admin_frontmatter_field_forbidden";
pub const ADMIN_API_VERSION_UNSUPPORTED: &str = "admin_api_version_unsupported";
pub const ADMIN_MDX_FORBIDDEN_PRESENTATION: &str = "admin_mdx_forbidden_presentation";
pub const ADMIN_SCENE_PHYSICAL_PATH_FORBIDDEN: &str = "admin_scene_physical_path_forbidden";
pub const ADMIN_SCENE_ROOT_MISSING: &str = "admin_scene_root_missing";
pub const ADMIN_SCENE_ROOT_DUPLICATE: &str = "admin_scene_root_duplicate";

const FRONTMATTER_FIELDS: &[&str] = &[
    "api_version",
    "title",
    "description",
    "menu",
    "parent",
    "order",
    "keywords",
    "default",
    "required_capabilities",
    "scope",
    "audit",
    "danger_level",
];

const IDENTITY_FIELDS: &[&str] = &["resource_id", "module_id"];
const PRESENTATION_FIELDS: &[&str] = &[
    "sections",
    "columns",
    "fields",
    "actions",
    "data",
    "provider",
    "validation",
];

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{location}: [{code}] {message}")]
pub struct AdminMdxError {
    pub path: Option<PathBuf>,
    pub line: usize,
    pub column: usize,
    pub code: String,
    pub message: String,
    location: String,
}

impl AdminMdxError {
    fn new(
        path: Option<&Path>,
        line: usize,
        column: usize,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let location = path.map_or_else(
            || format!("line {line}, column {column}"),
            |path| format!("{}:{line}:{column}", path.display()),
        );
        Self {
            path: path.map(Path::to_path_buf),
            line,
            column,
            code: code.into(),
            message: message.into(),
            location,
        }
    }

    fn io(path: &Path, error: std::io::Error) -> Self {
        Self::new(
            Some(path),
            1,
            1,
            ADMIN_MDX_PARSE,
            format!("failed to read admin entry: {error}"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxDocument {
    pub frontmatter: AdminMdxFrontmatter,
    pub visible_body: StageMarkdown,
    pub scene_use: String,
    pub fills: Vec<AdminMdxFill>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxFrontmatter {
    pub api_version: String,
    pub title: String,
    pub description: Option<String>,
    pub menu: Option<String>,
    pub parent: Option<String>,
    pub order: Option<i64>,
    pub keywords: Vec<String>,
    pub default: Option<bool>,
    pub required_capabilities: Vec<String>,
    pub scope: Option<String>,
    pub audit: Option<bool>,
    pub danger_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxFill {
    pub slot: String,
    pub content: String,
    pub source: Option<String>,
    pub line: usize,
}

pub fn parse_admin_mdx_source(source: &str) -> Result<AdminMdxDocument, AdminMdxError> {
    parse_admin_mdx_at(None, source)
}

pub fn parse_admin_mdx_file(path: &Path) -> Result<AdminMdxDocument, AdminMdxError> {
    let source = std::fs::read_to_string(path).map_err(|error| AdminMdxError::io(path, error))?;
    let mut document = parse_admin_mdx_at(Some(path), &source)?;
    document.source_path = Some(path.display().to_string().replace('\\', "/"));
    Ok(document)
}

fn parse_admin_mdx_at(
    path: Option<&Path>,
    source: &str,
) -> Result<AdminMdxDocument, AdminMdxError> {
    let lines: Vec<&str> = source.lines().collect();
    let frontmatter_end = find_frontmatter_end(path, &lines).map_err(map_stage_error)?;
    let values = parse_frontmatter(path, &lines[1..frontmatter_end], 2)?;
    let frontmatter = lower_frontmatter(path, &values)?;
    let (visible_body, scene_use, fills) = parse_body(path, &lines, frontmatter_end + 1)?;
    Ok(AdminMdxDocument {
        frontmatter,
        visible_body,
        scene_use,
        fills,
        source_path: None,
    })
}

fn parse_frontmatter(
    path: Option<&Path>,
    lines: &[&str],
    start_line: usize,
) -> Result<BTreeMap<String, (String, usize)>, AdminMdxError> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < lines.len() {
        let line_number = start_line + index;
        let line = lines[index].trim();
        if line.is_empty() {
            index += 1;
            continue;
        }
        let Some((key, raw_value)) = line.split_once(':') else {
            return Err(AdminMdxError::new(
                path,
                line_number,
                1,
                ADMIN_MDX_PARSE,
                "frontmatter entries must use `key: value`",
            ));
        };
        let key = key.trim();
        let code = if IDENTITY_FIELDS.contains(&key) {
            ADMIN_IDENTITY_REDECLARATION_FORBIDDEN
        } else if PRESENTATION_FIELDS.contains(&key) {
            ADMIN_MDX_FORBIDDEN_PRESENTATION
        } else {
            ADMIN_FRONTMATTER_FIELD_FORBIDDEN
        };
        if !FRONTMATTER_FIELDS.contains(&key) {
            return Err(AdminMdxError::new(
                path,
                line_number,
                1,
                code,
                format!("frontmatter field `{key}` is forbidden"),
            ));
        }
        let mut value = unquote(raw_value.trim());
        if value.is_empty() {
            let mut items = Vec::new();
            let mut cursor = index + 1;
            while cursor < lines.len() {
                let candidate = lines[cursor].trim();
                let Some(item) = candidate.strip_prefix("- ") else {
                    break;
                };
                items.push(unquote(item.trim()));
                cursor += 1;
            }
            if items.is_empty() {
                return Err(AdminMdxError::new(
                    path,
                    line_number,
                    key.len() + 2,
                    ADMIN_MDX_PARSE,
                    format!("frontmatter field `{key}` cannot be empty"),
                ));
            }
            value = items.join(",");
            index = cursor;
        } else {
            index += 1;
        }
        if values
            .insert(key.to_string(), (value, line_number))
            .is_some()
        {
            return Err(AdminMdxError::new(
                path,
                line_number,
                1,
                ADMIN_MDX_PARSE,
                format!("duplicate frontmatter field `{key}`"),
            ));
        }
    }
    Ok(values)
}

fn lower_frontmatter(
    path: Option<&Path>,
    values: &BTreeMap<String, (String, usize)>,
) -> Result<AdminMdxFrontmatter, AdminMdxError> {
    let required = |key: &str| {
        values
            .get(key)
            .map(|(value, _)| value.clone())
            .ok_or_else(|| {
                AdminMdxError::new(
                    path,
                    1,
                    1,
                    ADMIN_MDX_PARSE,
                    format!("missing required frontmatter field `{key}`"),
                )
            })
    };
    let api_version = required("api_version")?;
    if api_version != ADMIN_API_VERSION {
        return Err(AdminMdxError::new(
            path,
            values["api_version"].1,
            1,
            ADMIN_API_VERSION_UNSUPPORTED,
            format!("expected api_version `{ADMIN_API_VERSION}`, got `{api_version}`"),
        ));
    }
    let required_capabilities = list_value(values, "required_capabilities");
    if required_capabilities.is_empty() {
        return Err(AdminMdxError::new(
            path,
            1,
            1,
            ADMIN_MDX_PARSE,
            "`required_capabilities` must contain at least one capability",
        ));
    }
    let scope = optional(values, "scope");
    if scope.as_deref().is_some_and(|scope| scope != "app") {
        return Err(AdminMdxError::new(
            path,
            values["scope"].1,
            1,
            ADMIN_MDX_PARSE,
            "application Admin scope must be `app`",
        ));
    }
    let danger_level = optional(values, "danger_level");
    if danger_level
        .as_deref()
        .is_some_and(|value| !matches!(value, "normal" | "elevated" | "critical"))
    {
        return Err(AdminMdxError::new(
            path,
            values["danger_level"].1,
            1,
            ADMIN_MDX_PARSE,
            "`danger_level` must be normal, elevated, or critical",
        ));
    }
    Ok(AdminMdxFrontmatter {
        api_version,
        title: required("title")?,
        description: optional(values, "description"),
        menu: optional(values, "menu"),
        parent: optional(values, "parent"),
        order: optional_i64(path, values, "order")?,
        keywords: list_value(values, "keywords"),
        default: optional_bool(path, values, "default")?,
        required_capabilities,
        scope,
        audit: optional_bool(path, values, "audit")?,
        danger_level,
    })
}

fn parse_body(
    path: Option<&Path>,
    lines: &[&str],
    mut index: usize,
) -> Result<(StageMarkdown, String, Vec<AdminMdxFill>), AdminMdxError> {
    let mut prose = Vec::new();
    let mut scene_use = None;
    let mut fills = Vec::new();
    while index < lines.len() {
        let raw = lines[index];
        let trimmed = raw.trim();
        let line = index + 1;
        if let Some(args) = parse_named_directive_args(trimmed, "@scene") {
            ensure_only(path, line, &args, &["use"])?;
            let use_id = required_arg(path, line, &args, "scene", "use")?;
            if is_physical_scene_reference(&use_id) {
                return Err(AdminMdxError::new(
                    path,
                    line,
                    1,
                    ADMIN_SCENE_PHYSICAL_PATH_FORBIDDEN,
                    format!("`@scene` must use a stable root id, not `{use_id}`"),
                ));
            }
            if !validate_stable_scene_id(&use_id) {
                return Err(AdminMdxError::new(
                    path,
                    line,
                    1,
                    ADMIN_MDX_PARSE,
                    format!("invalid stable scene root id `{use_id}`"),
                ));
            }
            if scene_use.replace(use_id).is_some() {
                return Err(AdminMdxError::new(
                    path,
                    line,
                    1,
                    ADMIN_SCENE_ROOT_DUPLICATE,
                    "Admin Entry must contain exactly one `@scene` root",
                ));
            }
        } else if let Some(args) = parse_named_directive_args(trimmed, "@fill") {
            ensure_only(path, line, &args, &["slot", "content", "source"])?;
            let slot = required_arg(path, line, &args, "fill", "slot")?;
            let content = required_arg(path, line, &args, "fill", "content")?;
            let source = args.get("source").cloned();
            if !validate_id_token(&slot)
                || !validate_public_reference(&content)
                || source
                    .as_deref()
                    .is_some_and(|value| !validate_public_reference(value))
            {
                return Err(AdminMdxError::new(
                    path,
                    line,
                    1,
                    ADMIN_MDX_PARSE,
                    "`@fill` requires public slot/content/source ids",
                ));
            }
            fills.push(AdminMdxFill {
                slot,
                content,
                source,
                line,
            });
        } else if trimmed.starts_with('@') {
            let code = if ["@field", "@column", "@upload", "@action"]
                .iter()
                .any(|directive| trimmed.starts_with(directive))
            {
                ADMIN_MDX_FORBIDDEN_PRESENTATION
            } else {
                ADMIN_MDX_PARSE
            };
            return Err(AdminMdxError::new(
                path,
                line,
                1,
                code,
                format!("forbidden or malformed Admin directive `{trimmed}`"),
            ));
        } else {
            check_markdown_line(raw).map_err(|forbidden| {
                AdminMdxError::new(
                    path,
                    line,
                    1,
                    match forbidden {
                        MarkdownForbidden::JsxHtml => ADMIN_MDX_JSX_FORBIDDEN,
                        _ => ADMIN_MDX_JS_FORBIDDEN,
                    },
                    forbidden.message(),
                )
            })?;
            prose.push(raw);
        }
        index += 1;
    }
    let scene_use = scene_use.ok_or_else(|| {
        AdminMdxError::new(
            path,
            lines.len().max(1),
            1,
            ADMIN_SCENE_ROOT_MISSING,
            "Admin Entry requires exactly one `@scene(use=\"stable.id\")`",
        )
    })?;
    Ok((markdown_from_lines(&prose), scene_use, fills))
}

fn ensure_only(
    path: Option<&Path>,
    line: usize,
    args: &BTreeMap<String, String>,
    allowed: &[&str],
) -> Result<(), AdminMdxError> {
    if let Some(key) = args.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(AdminMdxError::new(
            path,
            line,
            1,
            ADMIN_MDX_PARSE,
            format!("unknown directive argument `{key}`"),
        ));
    }
    Ok(())
}

fn required_arg(
    path: Option<&Path>,
    line: usize,
    args: &BTreeMap<String, String>,
    directive: &str,
    key: &str,
) -> Result<String, AdminMdxError> {
    args.get(key).cloned().ok_or_else(|| {
        AdminMdxError::new(
            path,
            line,
            1,
            ADMIN_MDX_PARSE,
            format!("`@{directive}` requires `{key}=\"…\"`"),
        )
    })
}

fn is_physical_scene_reference(value: &str) -> bool {
    value.contains('/')
        || value.contains('\\')
        || value.starts_with('.')
        || value.ends_with(".mei")
        || Path::new(value).is_absolute()
}

fn validate_stable_scene_id(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(validate_id_token)
}

fn validate_public_reference(value: &str) -> bool {
    !is_physical_scene_reference(value) && value.split('.').all(validate_id_token)
}

fn optional(values: &BTreeMap<String, (String, usize)>, key: &str) -> Option<String> {
    values.get(key).map(|(value, _)| value.clone())
}

fn list_value(values: &BTreeMap<String, (String, usize)>, key: &str) -> Vec<String> {
    values
        .get(key)
        .map(|(value, _)| {
            value
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split([',', '|'])
                .map(str::trim)
                .map(|item| item.trim_matches(['"', '\'']))
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn optional_bool(
    path: Option<&Path>,
    values: &BTreeMap<String, (String, usize)>,
    key: &str,
) -> Result<Option<bool>, AdminMdxError> {
    let Some((value, line)) = values.get(key) else {
        return Ok(None);
    };
    match value.as_str() {
        "true" => Ok(Some(true)),
        "false" => Ok(Some(false)),
        _ => Err(AdminMdxError::new(
            path,
            *line,
            1,
            ADMIN_MDX_PARSE,
            format!("`{key}` must be true or false"),
        )),
    }
}

fn optional_i64(
    path: Option<&Path>,
    values: &BTreeMap<String, (String, usize)>,
    key: &str,
) -> Result<Option<i64>, AdminMdxError> {
    let Some((value, line)) = values.get(key) else {
        return Ok(None);
    };
    value.parse::<i64>().map(Some).map_err(|_| {
        AdminMdxError::new(
            path,
            *line,
            1,
            ADMIN_MDX_PARSE,
            format!("frontmatter `{key}` must be an integer"),
        )
    })
}

fn map_stage_error(error: crate::stage_mdx::StageMdxError) -> AdminMdxError {
    let code = match error.code.as_str() {
        crate::stage_mdx::codes::JSX_FORBIDDEN => ADMIN_MDX_JSX_FORBIDDEN,
        crate::stage_mdx::codes::JS_FORBIDDEN => ADMIN_MDX_JS_FORBIDDEN,
        _ => ADMIN_MDX_PARSE,
    };
    AdminMdxError::new(
        error.path.as_deref(),
        error.line,
        error.column,
        code,
        error.message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = r#"---
api_version: mei-admin-resource-v2
title: 单位信息
menu: 应用管理
required_capabilities:
  - config_upload
audit: true
---

维护单位信息。

@scene(use="admin.organization.overview")
@fill(slot="summary", content="organization.summary", source="organization.current")
"#;

    #[test]
    fn parses_v2_admin_entry() {
        let document = parse_admin_mdx_source(ENTRY).expect("admin entry");
        assert_eq!(document.frontmatter.api_version, ADMIN_API_VERSION);
        assert_eq!(document.scene_use, "admin.organization.overview");
        assert_eq!(document.fills.len(), 1);
        assert!(document.visible_body.markdown.contains("维护单位信息"));
    }

    #[test]
    fn rejects_identity_and_presentation_dsl() {
        let identity = ENTRY.replace("title:", "resource_id: organization\ntitle:");
        assert_eq!(
            parse_admin_mdx_source(&identity).unwrap_err().code,
            ADMIN_IDENTITY_REDECLARATION_FORBIDDEN
        );
        let field = format!("{ENTRY}\n@field(id=\"name\")");
        assert_eq!(
            parse_admin_mdx_source(&field).unwrap_err().code,
            ADMIN_MDX_FORBIDDEN_PRESENTATION
        );
        let route = ENTRY.replace("title:", "route: /admin/custom\ntitle:");
        assert_eq!(
            parse_admin_mdx_source(&route).unwrap_err().code,
            ADMIN_FRONTMATTER_FIELD_FORBIDDEN
        );
    }

    #[test]
    fn rejects_jsx_physical_path_and_duplicate_scene() {
        let jsx = ENTRY.replace("维护单位信息。", "<div />");
        assert_eq!(
            parse_admin_mdx_source(&jsx).unwrap_err().code,
            ADMIN_MDX_JSX_FORBIDDEN
        );
        let path = ENTRY.replace(
            "admin.organization.overview",
            "src/scene/admin/organization/overview.mei",
        );
        assert_eq!(
            parse_admin_mdx_source(&path).unwrap_err().code,
            ADMIN_SCENE_PHYSICAL_PATH_FORBIDDEN
        );
        let duplicate = format!("{ENTRY}\n@scene(use=\"admin.organization.other\")");
        assert_eq!(
            parse_admin_mdx_source(&duplicate).unwrap_err().code,
            ADMIN_SCENE_ROOT_DUPLICATE
        );
    }
}
