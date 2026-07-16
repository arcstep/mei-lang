use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const OBJECT_CATALOG_SCHEMA_VERSION: &str = "mei-object-catalog-v1";

/// Thin reference to a projection owned by another subsystem.
///
/// The catalog records only identity and routing metadata. Dataset rows,
/// metric definitions, scene payloads, and world geometry remain with their
/// respective owners.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectProjectionRef {
    pub role: String,
    pub kind: String,
    pub id: String,
    pub source_anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectIdentityMaterialization {
    /// Identity is computed from stable fields on a runtime dataset row.
    DatasetRow,
    /// Identity belongs to an explicitly declared object or non-row source.
    Declared,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectIdentityContract {
    pub materialization: ObjectIdentityMaterialization,
    pub fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalization: Option<String>,
}

/// Static contract for a domain object type.
///
/// Runtime dataset-row objects are materialized from `source` and `identity`;
/// this contract intentionally never stores the row payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectTypeContract {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub identity: ObjectIdentityContract,
    pub source: ObjectProjectionRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projections: Vec<ObjectProjectionRef>,
    pub source_anchor: String,
}

/// Runtime identity descriptor. It intentionally contains no dataset row
/// payload; consumers resolve non-identity data through `source_ref`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectDescriptor {
    pub object_id: String,
    pub object_type_id: String,
    pub identity_values: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<ObjectProjectionRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ObjectMaterializationError {
    #[error("object type `{object_type_id}` does not support dataset-row materialization")]
    NotDatasetRow { object_type_id: String },
    #[error("object type `{object_type_id}` has unknown normalization `{strategy}`")]
    UnknownNormalization {
        object_type_id: String,
        strategy: String,
    },
    #[error("object type `{object_type_id}` is missing identity field `{field}`")]
    MissingIdentityField {
        object_type_id: String,
        field: String,
        aliases: Vec<String>,
    },
    #[error("object type `{object_type_id}` identity field `{field}` is null")]
    NullIdentityValue {
        object_type_id: String,
        field: String,
    },
    #[error(
        "object type `{object_type_id}` identity field `{field}` must be a stable scalar, got {value_kind}"
    )]
    CompositeIdentityValue {
        object_type_id: String,
        field: String,
        value_kind: String,
    },
    #[error(
        "object type `{object_type_id}` identity field `{field}` conflicts with input `{conflicting_field}`"
    )]
    IdentityConflict {
        object_type_id: String,
        field: String,
        conflicting_field: String,
    },
    #[error("object catalog has no object type `{object_type_id}`")]
    UnknownObjectType { object_type_id: String },
}

impl ObjectTypeContract {
    pub fn materialize_dataset_row(
        &self,
        row: &Map<String, Value>,
    ) -> Result<ObjectDescriptor, ObjectMaterializationError> {
        if self.identity.materialization != ObjectIdentityMaterialization::DatasetRow {
            return Err(ObjectMaterializationError::NotDatasetRow {
                object_type_id: self.id.clone(),
            });
        }
        let normalization =
            IdentityNormalization::parse(self.identity.normalization.as_deref(), self.id.as_str())?;
        let mut identity_values = BTreeMap::new();
        let mut digest_values = Vec::with_capacity(self.identity.fields.len());

        for field in &self.identity.fields {
            let (normalized, canonical_digest) =
                self.resolve_identity_value(row, field, normalization)?;
            identity_values.insert(field.clone(), normalized);
            digest_values.push((field.as_str(), canonical_digest));
        }

        let object_id = object_id_for_identity(self.id.as_str(), &digest_values);
        Ok(ObjectDescriptor {
            object_id,
            object_type_id: self.id.clone(),
            identity_values,
            label: self.label.clone(),
            source_ref: Some(self.source.clone()),
        })
    }

    fn resolve_identity_value(
        &self,
        row: &Map<String, Value>,
        field: &str,
        normalization: IdentityNormalization,
    ) -> Result<(Value, String), ObjectMaterializationError> {
        if let Some(value) = row.get(field) {
            let resolved =
                normalize_identity_scalar(self.id.as_str(), field, value, normalization)?;
            if self.identity.fields.len() == 1 {
                self.reject_conflicting_aliases(row, field, &resolved.1, normalization)?;
            }
            return Ok(resolved);
        }

        if self.identity.fields.len() == 1 {
            let mut fallback = None::<(&str, (Value, String))>;
            for alias in &self.identity.aliases {
                let Some(value) = row.get(alias) else {
                    continue;
                };
                let resolved =
                    normalize_identity_scalar(self.id.as_str(), field, value, normalization)?;
                if let Some((_, (_, digest))) = &fallback {
                    if digest != &resolved.1 {
                        return Err(ObjectMaterializationError::IdentityConflict {
                            object_type_id: self.id.clone(),
                            field: field.to_string(),
                            conflicting_field: alias.clone(),
                        });
                    }
                } else {
                    fallback = Some((alias.as_str(), resolved));
                }
            }
            if let Some((_, resolved)) = fallback {
                return Ok(resolved);
            }
        }

        Err(ObjectMaterializationError::MissingIdentityField {
            object_type_id: self.id.clone(),
            field: field.to_string(),
            aliases: if self.identity.fields.len() == 1 {
                self.identity.aliases.clone()
            } else {
                Vec::new()
            },
        })
    }

    fn reject_conflicting_aliases(
        &self,
        row: &Map<String, Value>,
        field: &str,
        canonical_digest: &str,
        normalization: IdentityNormalization,
    ) -> Result<(), ObjectMaterializationError> {
        for alias in &self.identity.aliases {
            let Some(value) = row.get(alias) else {
                continue;
            };
            let (_, alias_digest) =
                normalize_identity_scalar(self.id.as_str(), field, value, normalization)?;
            if alias_digest != canonical_digest {
                return Err(ObjectMaterializationError::IdentityConflict {
                    object_type_id: self.id.clone(),
                    field: field.to_string(),
                    conflicting_field: alias.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectCatalog {
    pub schema_version: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<ObjectTypeContract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<ObjectProjectionRef>,
    pub source_anchor: String,
}

impl ObjectCatalog {
    pub fn object_type(&self, object_type_id: &str) -> Option<&ObjectTypeContract> {
        self.types
            .iter()
            .find(|object_type| object_type.id == object_type_id)
    }

    pub fn materialize_dataset_row(
        &self,
        object_type_id: &str,
        row: &Map<String, Value>,
    ) -> Result<ObjectDescriptor, ObjectMaterializationError> {
        self.object_type(object_type_id)
            .ok_or_else(|| ObjectMaterializationError::UnknownObjectType {
                object_type_id: object_type_id.to_string(),
            })?
            .materialize_dataset_row(row)
    }
}

#[derive(Debug, Clone, Copy)]
enum IdentityNormalization {
    None,
    Trim,
}

impl IdentityNormalization {
    fn parse(
        strategy: Option<&str>,
        object_type_id: &str,
    ) -> Result<Self, ObjectMaterializationError> {
        match strategy {
            None | Some("none") => Ok(Self::None),
            Some("trim") => Ok(Self::Trim),
            Some(strategy) => Err(ObjectMaterializationError::UnknownNormalization {
                object_type_id: object_type_id.to_string(),
                strategy: strategy.to_string(),
            }),
        }
    }
}

fn normalize_identity_scalar(
    object_type_id: &str,
    field: &str,
    value: &Value,
    normalization: IdentityNormalization,
) -> Result<(Value, String), ObjectMaterializationError> {
    match value {
        Value::Null => Err(ObjectMaterializationError::NullIdentityValue {
            object_type_id: object_type_id.to_string(),
            field: field.to_string(),
        }),
        Value::String(value) => {
            let value = match normalization {
                IdentityNormalization::None => value.clone(),
                IdentityNormalization::Trim => value.trim().to_string(),
            };
            Ok((Value::String(value.clone()), format!("s:{value}")))
        }
        Value::Number(value) => Ok((Value::Number(value.clone()), format!("n:{value}"))),
        Value::Bool(value) => Ok((Value::Bool(*value), format!("b:{value}"))),
        Value::Array(_) => Err(ObjectMaterializationError::CompositeIdentityValue {
            object_type_id: object_type_id.to_string(),
            field: field.to_string(),
            value_kind: "array".to_string(),
        }),
        Value::Object(_) => Err(ObjectMaterializationError::CompositeIdentityValue {
            object_type_id: object_type_id.to_string(),
            field: field.to_string(),
            value_kind: "object".to_string(),
        }),
    }
}

fn object_id_for_identity(object_type_id: &str, fields: &[(&str, String)]) -> String {
    let mut hasher = Sha256::new();
    hash_len_prefixed(&mut hasher, OBJECT_CATALOG_SCHEMA_VERSION.as_bytes());
    hash_len_prefixed(&mut hasher, object_type_id.as_bytes());
    for (field, value) in fields {
        hash_len_prefixed(&mut hasher, field.as_bytes());
        hash_len_prefixed(&mut hasher, value.as_bytes());
    }
    format!("obj_{:x}", hasher.finalize())
}

fn hash_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_catalog_serde_is_stable_and_row_free() {
        let catalog = ObjectCatalog {
            schema_version: OBJECT_CATALOG_SCHEMA_VERSION.to_string(),
            id: "warning_objects".to_string(),
            types: vec![ObjectTypeContract {
                id: "zhifa.Warning".to_string(),
                label: Some("Warning".to_string()),
                identity: ObjectIdentityContract {
                    materialization: ObjectIdentityMaterialization::DatasetRow,
                    fields: vec!["warning_id".to_string()],
                    aliases: vec!["warningId".to_string()],
                    normalization: Some("trim".to_string()),
                },
                source: ObjectProjectionRef {
                    role: "source".to_string(),
                    kind: "dataset_ref".to_string(),
                    id: "warning_rows".to_string(),
                    source_anchor: "domain/warnings.objects.mei".to_string(),
                },
                capabilities: vec!["select".to_string(), "explain".to_string()],
                projections: vec![ObjectProjectionRef {
                    role: "label".to_string(),
                    kind: "field_ref".to_string(),
                    id: "title".to_string(),
                    source_anchor: "domain/warnings.objects.mei".to_string(),
                }],
                source_anchor: "domain/warnings.objects.mei".to_string(),
            }],
            refs: vec![],
            source_anchor: "domain/warnings.objects.mei".to_string(),
        };

        let value = serde_json::to_value(&catalog).expect("serialize catalog");
        assert_eq!(value["schema_version"], OBJECT_CATALOG_SCHEMA_VERSION);
        assert_eq!(value["types"][0]["identity"]["fields"][0], "warning_id");
        assert_eq!(
            value["types"][0]["identity"]["materialization"],
            "dataset_row"
        );
        assert_eq!(value["types"][0]["identity"]["aliases"][0], "warningId");
        assert_eq!(value["types"][0]["capabilities"][1], "explain");
        assert_eq!(value["types"][0]["source"]["kind"], "dataset_ref");
        assert!(value["types"][0].get("rows").is_none());

        let decoded: ObjectCatalog = serde_json::from_value(value).expect("deserialize catalog");
        assert_eq!(decoded, catalog);
    }

    #[test]
    fn dataset_row_materialization_is_stable_alias_aware_and_row_free() {
        let object_type = warning_type();
        let canonical_row = json!({
            "预警ID": " W-001 ",
            "title": "首次预警",
            "severity": 1
        });
        let alias_row = json!({
            "warning_id": "W-001",
            "title": "标题已变化",
            "severity": 9
        });
        let changed_non_identity_row = json!({
            "预警ID": "W-001",
            "title": "另一个标题",
            "extra": {"not": "retained"}
        });
        let changed_identity_row = json!({
            "预警ID": "W-002",
            "title": "另一条预警"
        });

        let canonical = object_type
            .materialize_dataset_row(canonical_row.as_object().expect("row"))
            .expect("canonical identity");
        let alias = object_type
            .materialize_dataset_row(alias_row.as_object().expect("row"))
            .expect("alias fallback");
        let changed_non_identity = object_type
            .materialize_dataset_row(changed_non_identity_row.as_object().expect("row"))
            .expect("changed non-identity");
        let changed_identity = object_type
            .materialize_dataset_row(changed_identity_row.as_object().expect("row"))
            .expect("changed identity");

        assert_eq!(canonical.object_id, alias.object_id);
        assert_eq!(canonical.object_id, changed_non_identity.object_id);
        assert_ne!(canonical.object_id, changed_identity.object_id);
        assert!(canonical.object_id.starts_with("obj_"));
        assert_eq!(canonical.object_id.len(), 68);
        assert_eq!(
            canonical.identity_values.get("预警ID"),
            Some(&json!("W-001"))
        );

        let descriptor = serde_json::to_value(&canonical).expect("descriptor serde");
        assert!(descriptor.get("row").is_none());
        assert!(descriptor.get("payload").is_none());
        assert!(descriptor.get("title").is_none());
        assert_eq!(descriptor["source_ref"]["id"], "warning_rows");
    }

    #[test]
    fn dataset_row_materialization_reports_structured_identity_errors() {
        let object_type = warning_type();

        let missing = object_type
            .materialize_dataset_row(json!({"title": "missing"}).as_object().expect("row"))
            .expect_err("missing identity");
        assert!(matches!(
            missing,
            ObjectMaterializationError::MissingIdentityField { .. }
        ));

        let null = object_type
            .materialize_dataset_row(json!({"预警ID": null}).as_object().expect("row"))
            .expect_err("null identity");
        assert!(matches!(
            null,
            ObjectMaterializationError::NullIdentityValue { .. }
        ));

        let composite = object_type
            .materialize_dataset_row(json!({"预警ID": ["W-001"]}).as_object().expect("row"))
            .expect_err("composite identity");
        assert!(matches!(
            composite,
            ObjectMaterializationError::CompositeIdentityValue { .. }
        ));

        let conflict = object_type
            .materialize_dataset_row(
                json!({"预警ID": "W-001", "warning_id": "W-002"})
                    .as_object()
                    .expect("row"),
            )
            .expect_err("conflicting identity");
        assert!(matches!(
            &conflict,
            ObjectMaterializationError::IdentityConflict { .. }
        ));
        let diagnostic = serde_json::to_value(conflict).expect("error serde");
        assert_eq!(diagnostic["code"], "identity_conflict");

        let mut unknown_normalization = object_type.clone();
        unknown_normalization.identity.normalization = Some("lowercase".to_string());
        let error = unknown_normalization
            .materialize_dataset_row(json!({"预警ID": "W-001"}).as_object().expect("row"))
            .expect_err("unknown normalization");
        assert!(matches!(
            error,
            ObjectMaterializationError::UnknownNormalization { .. }
        ));
    }

    #[test]
    fn declared_type_and_unknown_catalog_type_cannot_materialize_dataset_rows() {
        let mut declared = warning_type();
        declared.identity.materialization = ObjectIdentityMaterialization::Declared;
        let row = json!({"预警ID": "W-001"});
        let error = declared
            .materialize_dataset_row(row.as_object().expect("row"))
            .expect_err("declared type must reject rows");
        assert!(matches!(
            error,
            ObjectMaterializationError::NotDatasetRow { .. }
        ));

        let catalog = ObjectCatalog {
            schema_version: OBJECT_CATALOG_SCHEMA_VERSION.to_string(),
            id: "warning_objects".to_string(),
            types: vec![warning_type()],
            refs: Vec::new(),
            source_anchor: "domain/warnings.objects.mei".to_string(),
        };
        assert!(catalog.object_type("zhifa.Warning").is_some());
        let descriptor = catalog
            .materialize_dataset_row("zhifa.Warning", row.as_object().expect("row"))
            .expect("catalog materialization");
        assert_eq!(descriptor.object_type_id, "zhifa.Warning");
        let error = catalog
            .materialize_dataset_row("zhifa.Missing", row.as_object().expect("row"))
            .expect_err("unknown type");
        assert!(matches!(
            error,
            ObjectMaterializationError::UnknownObjectType { .. }
        ));
    }

    fn warning_type() -> ObjectTypeContract {
        ObjectTypeContract {
            id: "zhifa.Warning".to_string(),
            label: Some("Warning".to_string()),
            identity: ObjectIdentityContract {
                materialization: ObjectIdentityMaterialization::DatasetRow,
                fields: vec!["预警ID".to_string()],
                aliases: vec!["warning_id".to_string()],
                normalization: Some("trim".to_string()),
            },
            source: ObjectProjectionRef {
                role: "source".to_string(),
                kind: "dataset_ref".to_string(),
                id: "warning_rows".to_string(),
                source_anchor: "domain/warnings.objects.mei".to_string(),
            },
            capabilities: vec!["select".to_string()],
            projections: Vec::new(),
            source_anchor: "domain/warnings.objects.mei".to_string(),
        }
    }
}
