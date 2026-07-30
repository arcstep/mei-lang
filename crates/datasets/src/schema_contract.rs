//! Dataset schema ↔ physical snapshot column contract.
//!
//! `column(..., source = "…")` is an authored mapping onto parquet/xlsx headers.
//! Missing required physical columns must fail loudly; optional may stay NULL.

use anyhow::{bail, Result};
use mei_lang_kernel::ColumnSchema;

/// Result of checking schema physical sources against snapshot/header columns.
#[derive(Debug, Clone, Default)]
pub struct SchemaPhysicalSourceCheck {
    pub missing_required: Vec<MissingPhysicalSource>,
    pub missing_optional: Vec<MissingPhysicalSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingPhysicalSource {
    pub logic_column: String,
    pub physical_column: String,
}

impl SchemaPhysicalSourceCheck {
    pub fn has_required_failures(&self) -> bool {
        !self.missing_required.is_empty()
    }
}

/// Compare authored schema sources to physical columns (parquet / import-manifest).
pub fn check_schema_physical_sources(
    schema: &[ColumnSchema],
    physical_columns: &[String],
) -> SchemaPhysicalSourceCheck {
    let mut out = SchemaPhysicalSourceCheck::default();
    if schema.is_empty() || physical_columns.is_empty() {
        return out;
    }
    for col in schema {
        let physical = col
            .source
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(col.name.as_str());
        if physical_columns.iter().any(|name| name == physical) {
            continue;
        }
        let missing = MissingPhysicalSource {
            logic_column: col.name.clone(),
            physical_column: physical.to_string(),
        };
        if col.optional {
            out.missing_optional.push(missing);
        } else {
            out.missing_required.push(missing);
        }
    }
    out
}

/// Fail when any non-optional schema source is absent from physical columns.
pub fn ensure_schema_physical_sources(
    dataset_id: &str,
    schema: &[ColumnSchema],
    physical_columns: &[String],
    source_path: Option<&str>,
) -> Result<SchemaPhysicalSourceCheck> {
    let check = check_schema_physical_sources(schema, physical_columns);
    for miss in &check.missing_optional {
        tracing::warn!(
            dataset_id = %dataset_id,
            source_path = source_path.unwrap_or(""),
            logic_column = %miss.logic_column,
            physical_column = %miss.physical_column,
            "optional dataset schema.source missing from snapshot columns; will project NULL"
        );
    }
    if check.has_required_failures() {
        let detail = check
            .missing_required
            .iter()
            .map(|m| format!("{}←{}", m.logic_column, m.physical_column))
            .collect::<Vec<_>>()
            .join(", ");
        let sample = physical_columns
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        bail!(
            "dataset `{}` schema.source missing from snapshot columns [{}] (source=`{}`); physical columns sample: [{}]. Update dataset.schema or the source headers, then re-run prebuild.",
            dataset_id,
            detail,
            source_path.unwrap_or(""),
            sample
        );
    }
    Ok(check)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mei_lang_kernel::ColumnSchema;

    fn col(name: &str, source: Option<&str>, optional: bool) -> ColumnSchema {
        ColumnSchema {
            name: name.to_string(),
            type_name: "string".into(),
            source: source.map(str::to_string),
            optional,
            unit: None,
            normalize: None,
        }
    }

    #[test]
    fn required_missing_source_fails() {
        let schema = vec![
            col("行权类别", Some("监督类别"), false),
            col("存在的问题", Some("存在的问题"), true),
        ];
        let physical = vec!["行权类别".into(), "存在的问题".into()];
        let check = check_schema_physical_sources(&schema, &physical);
        assert!(check.has_required_failures());
        assert_eq!(check.missing_required[0].physical_column, "监督类别");
        assert!(ensure_schema_physical_sources("matters", &schema, &physical, Some("a.xls")).is_err());
    }

    #[test]
    fn optional_missing_source_ok_with_warning_slot() {
        let schema = vec![col("视频", Some("视频路径"), true)];
        let physical = vec!["序号".into()];
        let check = check_schema_physical_sources(&schema, &physical);
        assert!(!check.has_required_failures());
        assert_eq!(check.missing_optional.len(), 1);
        assert!(ensure_schema_physical_sources("ds", &schema, &physical, None).is_ok());
    }

    #[test]
    fn aligned_source_passes() {
        let schema = vec![col("行权类别", Some("行权类别"), false)];
        let physical = vec!["行权类别".into()];
        assert!(!check_schema_physical_sources(&schema, &physical).has_required_failures());
    }
}
