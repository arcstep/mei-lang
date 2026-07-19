//! Restricted Admin Page MDX front-end (`src/admin/**/*.admin.mdx`).
//!
//! The syntax is intentionally shallower than Scene `.mei`: frontmatter owns
//! resource governance while body directives describe Page/Form blocks.
//! JSX, HTML, JavaScript expressions, and arbitrary directives are rejected.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::stage_mdx::{
    check_markdown_line, find_frontmatter_end, markdown_from_lines, parse_frontmatter_map,
    parse_heading, parse_named_directive_args, validate_id_token, StageMarkdown,
};

pub const ADMIN_MDX_PARSE: &str = "admin_mdx_parse";
pub const ADMIN_MDX_JSX_FORBIDDEN: &str = "admin_mdx_jsx_forbidden";
pub const ADMIN_MDX_JS_FORBIDDEN: &str = "admin_mdx_js_forbidden";

const FRONTMATTER_FIELDS: &[&str] = &[
    "resource_id",
    "title",
    "description",
    "template",
    "provider",
    "record_path",
    "config_path",
    "required_capabilities",
    "scope",
    "audit",
    "danger_level",
    "revision_policy",
    "dirty_policy",
    "apply_policy",
    "navigation_menu",
    "navigation_parent",
    "navigation_order",
    "navigation_keywords",
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
            format!("failed to read admin mdx: {error}"),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxDocument {
    pub frontmatter: AdminMdxFrontmatter,
    pub blocks: Vec<AdminMdxBlock>,
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxFrontmatter {
    pub resource_id: String,
    pub title: String,
    pub description: Option<String>,
    pub template: String,
    pub provider: String,
    pub record_path: Option<String>,
    pub config_path: Option<String>,
    pub required_capabilities: Vec<String>,
    pub scope: Option<String>,
    pub audit: Option<bool>,
    pub danger_level: Option<String>,
    pub revision_policy: Option<String>,
    pub dirty_policy: Option<String>,
    pub apply_policy: Option<String>,
    pub navigation_menu: Option<String>,
    pub navigation_parent: Option<String>,
    pub navigation_order: Option<i64>,
    pub navigation_keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdminMdxBlock {
    Markdown {
        content: StageMarkdown,
    },
    Section {
        id: String,
        title: String,
        fields: Vec<AdminMdxField>,
    },
    Column(AdminMdxColumn),
    Upload(AdminMdxUpload),
    Action(AdminMdxAction),
    Readonly(AdminMdxReadonly),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxField {
    pub id: String,
    pub path: Option<String>,
    pub label: String,
    pub control: String,
    pub required: Option<bool>,
    pub readonly: Option<bool>,
    pub description: Option<String>,
    pub options: Vec<AdminMdxOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxColumn {
    pub id: String,
    pub label: String,
    pub control: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxUpload {
    pub accept: Vec<String>,
    pub max_bytes: Option<u64>,
    pub replace_modes: Vec<String>,
    pub retain_versions: Option<bool>,
    pub schema_ref: Option<String>,
    pub requires_review: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxAction {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub method: String,
    pub danger_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminMdxReadonly {
    pub content_kind: String,
    pub id: String,
    pub title: Option<String>,
    pub data_ref: Option<String>,
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
    let values = parse_frontmatter_map(path, &lines[1..frontmatter_end], 2, FRONTMATTER_FIELDS)
        .map_err(map_stage_error)?;
    let frontmatter = lower_frontmatter(path, &values)?;
    let blocks = parse_body(path, &lines, frontmatter_end + 1)?;
    Ok(AdminMdxDocument {
        frontmatter,
        blocks,
        source_path: None,
    })
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
    let resource_id = required("resource_id")?;
    if !validate_id_token(&resource_id) {
        return Err(AdminMdxError::new(
            path,
            values["resource_id"].1,
            1,
            ADMIN_MDX_PARSE,
            format!("invalid resource_id `{resource_id}`"),
        ));
    }

    Ok(AdminMdxFrontmatter {
        resource_id,
        title: required("title")?,
        description: optional(values, "description"),
        template: required("template")?,
        provider: required("provider")?,
        record_path: optional(values, "record_path"),
        config_path: optional(values, "config_path"),
        required_capabilities: list_value(values, "required_capabilities"),
        scope: optional(values, "scope"),
        audit: optional_bool(path, values, "audit")?,
        danger_level: optional(values, "danger_level"),
        revision_policy: optional(values, "revision_policy"),
        dirty_policy: optional(values, "dirty_policy"),
        apply_policy: optional(values, "apply_policy"),
        navigation_menu: optional(values, "navigation_menu"),
        navigation_parent: optional(values, "navigation_parent"),
        navigation_order: optional_i64(path, values, "navigation_order")?,
        navigation_keywords: list_value(values, "navigation_keywords"),
    })
}

fn parse_body(
    path: Option<&Path>,
    lines: &[&str],
    mut index: usize,
) -> Result<Vec<AdminMdxBlock>, AdminMdxError> {
    let mut blocks = Vec::new();
    while index < lines.len() {
        let trimmed = lines[index].trim();
        let line = index + 1;
        if trimmed.is_empty() {
            index += 1;
            continue;
        }
        if let Some((title, id)) = parse_heading(trimmed, 2) {
            if !validate_id_token(&id) {
                return Err(AdminMdxError::new(
                    path,
                    line,
                    1,
                    ADMIN_MDX_PARSE,
                    format!("invalid section id `{id}`"),
                ));
            }
            index += 1;
            let mut fields = Vec::new();
            while index < lines.len() {
                let candidate = lines[index].trim();
                if candidate.is_empty() {
                    index += 1;
                    continue;
                }
                let Some(args) = parse_named_directive_args(candidate, "@field") else {
                    break;
                };
                fields.push(parse_field(path, index + 1, &args)?);
                index += 1;
            }
            blocks.push(AdminMdxBlock::Section { id, title, fields });
            continue;
        }
        if let Some(args) = parse_named_directive_args(trimmed, "@column") {
            blocks.push(AdminMdxBlock::Column(parse_column(path, line, &args)?));
            index += 1;
            continue;
        }
        if let Some(args) = parse_named_directive_args(trimmed, "@upload") {
            blocks.push(AdminMdxBlock::Upload(parse_upload(path, line, &args)?));
            index += 1;
            continue;
        }
        if let Some(args) = parse_named_directive_args(trimmed, "@action") {
            blocks.push(AdminMdxBlock::Action(parse_action(path, line, &args)?));
            index += 1;
            continue;
        }
        let mut readonly_handled = false;
        for (directive, kind) in [
            ("@readonly_content", "content"),
            ("@readonly_chart", "chart"),
            ("@readonly_canvas", "canvas"),
        ] {
            if let Some(args) = parse_named_directive_args(trimmed, directive) {
                blocks.push(AdminMdxBlock::Readonly(parse_readonly(
                    path, line, kind, &args,
                )?));
                index += 1;
                readonly_handled = true;
                break;
            }
        }
        if readonly_handled {
            continue;
        }
        if trimmed.starts_with('@') {
            return Err(AdminMdxError::new(
                path,
                line,
                1,
                ADMIN_MDX_PARSE,
                format!("unknown or malformed admin directive `{trimmed}`"),
            ));
        }

        let start = index;
        while index < lines.len() {
            let candidate = lines[index].trim();
            if candidate.starts_with('@') || parse_heading(candidate, 2).is_some() {
                break;
            }
            if let Err(forbidden) = check_markdown_line(lines[index]) {
                return Err(AdminMdxError::new(
                    path,
                    index + 1,
                    1,
                    match forbidden {
                        crate::stage_mdx::MarkdownForbidden::JsxHtml => ADMIN_MDX_JSX_FORBIDDEN,
                        _ => ADMIN_MDX_JS_FORBIDDEN,
                    },
                    forbidden.message(),
                ));
            }
            index += 1;
        }
        blocks.push(AdminMdxBlock::Markdown {
            content: markdown_from_lines(&lines[start..index]),
        });
    }
    Ok(blocks)
}

fn parse_field(
    path: Option<&Path>,
    line: usize,
    args: &BTreeMap<String, String>,
) -> Result<AdminMdxField, AdminMdxError> {
    ensure_only(
        path,
        line,
        args,
        &[
            "id",
            "path",
            "label",
            "control",
            "required",
            "readonly",
            "description",
            "options",
        ],
    )?;
    Ok(AdminMdxField {
        id: required_arg(path, line, args, "field", "id")?,
        path: args.get("path").cloned(),
        label: required_arg(path, line, args, "field", "label")?,
        control: required_arg(path, line, args, "field", "control")?,
        required: arg_bool(path, line, args, "required")?,
        readonly: arg_bool(path, line, args, "readonly")?,
        description: args.get("description").cloned(),
        options: parse_options(args.get("options").map(String::as_str).unwrap_or_default()),
    })
}

fn parse_column(
    path: Option<&Path>,
    line: usize,
    args: &BTreeMap<String, String>,
) -> Result<AdminMdxColumn, AdminMdxError> {
    ensure_only(path, line, args, &["id", "label", "control"])?;
    Ok(AdminMdxColumn {
        id: required_arg(path, line, args, "column", "id")?,
        label: required_arg(path, line, args, "column", "label")?,
        control: args.get("control").cloned(),
    })
}

fn parse_upload(
    path: Option<&Path>,
    line: usize,
    args: &BTreeMap<String, String>,
) -> Result<AdminMdxUpload, AdminMdxError> {
    ensure_only(
        path,
        line,
        args,
        &[
            "accept",
            "max_bytes",
            "replace_modes",
            "retain_versions",
            "schema_ref",
            "requires_review",
        ],
    )?;
    Ok(AdminMdxUpload {
        accept: split_list(args.get("accept").map(String::as_str).unwrap_or_default()),
        max_bytes: arg_u64(path, line, args, "max_bytes")?,
        replace_modes: split_list(
            args.get("replace_modes")
                .map(String::as_str)
                .unwrap_or_default(),
        ),
        retain_versions: arg_bool(path, line, args, "retain_versions")?,
        schema_ref: args.get("schema_ref").cloned(),
        requires_review: arg_bool(path, line, args, "requires_review")?,
    })
}

fn parse_action(
    path: Option<&Path>,
    line: usize,
    args: &BTreeMap<String, String>,
) -> Result<AdminMdxAction, AdminMdxError> {
    ensure_only(
        path,
        line,
        args,
        &["id", "label", "provider", "method", "danger_level"],
    )?;
    Ok(AdminMdxAction {
        id: required_arg(path, line, args, "action", "id")?,
        label: required_arg(path, line, args, "action", "label")?,
        provider: required_arg(path, line, args, "action", "provider")?,
        method: required_arg(path, line, args, "action", "method")?,
        danger_level: args.get("danger_level").cloned(),
    })
}

fn parse_readonly(
    path: Option<&Path>,
    line: usize,
    content_kind: &str,
    args: &BTreeMap<String, String>,
) -> Result<AdminMdxReadonly, AdminMdxError> {
    ensure_only(path, line, args, &["id", "title", "data_ref"])?;
    Ok(AdminMdxReadonly {
        content_kind: content_kind.to_string(),
        id: required_arg(path, line, args, "readonly", "id")?,
        title: args.get("title").cloned(),
        data_ref: args.get("data_ref").cloned(),
    })
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

fn optional(values: &BTreeMap<String, (String, usize)>, key: &str) -> Option<String> {
    values.get(key).map(|(value, _)| value.clone())
}

fn list_value(values: &BTreeMap<String, (String, usize)>, key: &str) -> Vec<String> {
    values
        .get(key)
        .map(|(value, _)| split_list(value))
        .unwrap_or_default()
}

fn split_list(value: &str) -> Vec<String> {
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
}

fn parse_options(value: &str) -> Vec<AdminMdxOption> {
    split_list(value)
        .into_iter()
        .map(|entry| {
            let (value, label) = entry
                .split_once('=')
                .map_or_else(|| (entry.as_str(), entry.as_str()), |pair| pair);
            AdminMdxOption {
                value: value.trim().to_string(),
                label: label.trim().to_string(),
            }
        })
        .collect()
}

fn optional_bool(
    path: Option<&Path>,
    values: &BTreeMap<String, (String, usize)>,
    key: &str,
) -> Result<Option<bool>, AdminMdxError> {
    let Some((value, line)) = values.get(key) else {
        return Ok(None);
    };
    parse_bool(path, *line, key, value).map(Some)
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

fn arg_bool(
    path: Option<&Path>,
    line: usize,
    args: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<bool>, AdminMdxError> {
    args.get(key)
        .map(|value| parse_bool(path, line, key, value))
        .transpose()
}

fn arg_u64(
    path: Option<&Path>,
    line: usize,
    args: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<u64>, AdminMdxError> {
    args.get(key)
        .map(|value| {
            value.parse::<u64>().map_err(|_| {
                AdminMdxError::new(
                    path,
                    line,
                    1,
                    ADMIN_MDX_PARSE,
                    format!("`{key}` must be a non-negative integer"),
                )
            })
        })
        .transpose()
}

fn parse_bool(
    path: Option<&Path>,
    line: usize,
    key: &str,
    value: &str,
) -> Result<bool, AdminMdxError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(AdminMdxError::new(
            path,
            line,
            1,
            ADMIN_MDX_PARSE,
            format!("`{key}` must be true or false"),
        )),
    }
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

    const FORM: &str = r#"---
resource_id: organization
title: 单位信息
template: singleton-form
provider: config-record
record_path: admin/data/organization.json
required_capabilities: [config_upload]
scope: app
revision_policy: optimistic
dirty_policy: block-leave
audit: true
navigation_menu: 管理
navigation_order: 10
---

单位基础信息。

## 基本信息 {#basic}
@field(id="name", label="单位名称", control="text", required=true)
@field(id="contact", label="联系人", control="text")
"#;

    #[test]
    fn parses_form_page() {
        let document = parse_admin_mdx_source(FORM).expect("admin mdx");
        assert_eq!(document.frontmatter.resource_id, "organization");
        assert_eq!(
            document.frontmatter.required_capabilities,
            vec!["config_upload"]
        );
        assert!(document
            .blocks
            .iter()
            .any(|block| matches!(block, AdminMdxBlock::Markdown { .. })));
        let section = document
            .blocks
            .iter()
            .find_map(|block| match block {
                AdminMdxBlock::Section { fields, .. } => Some(fields),
                _ => None,
            })
            .expect("section");
        assert_eq!(section.len(), 2);
        assert_eq!(section[0].id, "name");
    }

    #[test]
    fn rejects_jsx() {
        let source = format!("{FORM}\n<div />\n");
        let error = parse_admin_mdx_source(&source).expect_err("jsx");
        assert_eq!(error.code, ADMIN_MDX_JSX_FORBIDDEN);
    }
}
