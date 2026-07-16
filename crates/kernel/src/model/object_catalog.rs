use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub const OBJECT_CATALOG_SCHEMA_VERSION: &str = "mei-object-catalog-v1";
pub const OBJECT_INDEX_ENTRY_KIND: &str = "internal_object_index";
pub const DEFAULT_OBJECT_ASSEMBLY_KIND: &str = "default_object_assembly";
pub const INTERACTION_PROTOCOL_SCHEMA_VERSION: &str = "mei-interaction-v1";
pub const OBJECT_RECIPE_SCHEMA_VERSION: &str = "mei-stock-object-recipe-v1";

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<ObjectProjectionRef>,
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
    pub intent_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectCatalogAuthoringMode {
    Legacy,
    AuthorIntent,
}

impl Default for ObjectCatalogAuthoringMode {
    fn default() -> Self {
        Self::Legacy
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectCatalogDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub source_anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectRecipeSlotRequirement {
    Required,
    Optional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectRecipeProjectionState {
    Ready,
    Degraded,
    Placeholder,
    Hidden,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectRecipeSlotContract {
    pub name: String,
    pub requirement: ObjectRecipeSlotRequirement,
    pub missing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectRecipeProjectionContract {
    pub role: String,
    pub id: String,
    pub required_slots: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub optional_slots: Vec<String>,
    pub partial_behavior: ObjectRecipeProjectionState,
    pub absent_behavior: ObjectRecipeProjectionState,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reuses: Vec<ObjectProjectionRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectRecipeInteractionContract {
    pub trigger: String,
    pub intents: Vec<InteractionIntent>,
    pub subject_kind: String,
    pub projection_role: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_slots: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectRecipeResponderContract {
    pub role: String,
    pub intents: Vec<InteractionIntent>,
    pub projection_role: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_slots: Vec<String>,
}

/// Stable stock recipe metadata. It contains only slot names, fallback rules,
/// interaction intents, and references to existing projection components.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectRecipeContract {
    pub schema_version: String,
    pub id: String,
    pub slots: Vec<ObjectRecipeSlotContract>,
    pub projections: Vec<ObjectRecipeProjectionContract>,
    pub interactions: Vec<ObjectRecipeInteractionContract>,
    pub responders: Vec<ObjectRecipeResponderContract>,
    pub override_precedence: Vec<String>,
    pub identity_locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy_notice: Option<String>,
    pub source_anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectRecipeProjectionAssembly {
    pub role: String,
    pub projection: ObjectProjectionRef,
    pub state: ObjectRecipeProjectionState,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, ObjectProjectionRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_slots: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reuses: Vec<ObjectProjectionRef>,
}

/// Author-authored object intent. All nested values are thin references or
/// explicit override configuration; owner payloads remain in their source
/// subsystems.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectIntent {
    pub intent_id: String,
    pub object_type_id: String,
    pub source: ObjectProjectionRef,
    pub identity: ObjectIdentityContract,
    pub recipe: ObjectProjectionRef,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, ObjectProjectionRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub relations: BTreeMap<String, Vec<ObjectProjectionRef>>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "override")]
    pub override_props: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owner_hints: Vec<ObjectProjectionRef>,
    pub source_anchor: String,
}

/// Deterministic internal lookup entry derived from an [`ObjectIntent`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectIndexEntry {
    pub kind: String,
    pub key: String,
    pub intent_id: String,
    pub object_type_id: String,
    pub source: ObjectProjectionRef,
    pub identity: ObjectProjectionRef,
    pub recipe: ObjectProjectionRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owner_hints: Vec<ObjectProjectionRef>,
    pub source_anchor: String,
}

/// Default assembly generated from intent references. It is an assembly plan,
/// never a copy of source, slot, relation, or recipe payloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultObjectAssembly {
    pub kind: String,
    pub id: String,
    pub intent_id: String,
    pub recipe: ObjectProjectionRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_contract: Option<ObjectRecipeContract>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub slots: BTreeMap<String, ObjectProjectionRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub relations: BTreeMap<String, Vec<ObjectProjectionRef>>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "override")]
    pub override_props: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_override: Option<Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub override_sources: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projections: Vec<ObjectRecipeProjectionAssembly>,
    pub source_anchor: String,
}

/// The closed set of platform interaction intents. Keeping this as an enum
/// makes event serialization stable and prevents component-local verb drift.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum InteractionIntent {
    Select,
    OpenProjection,
    ExplainMetric,
    FilterQuery,
    FocusViewpoint,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectFocusCardinality {
    Single,
    Multiple,
}

/// Canonical focus over one or more concrete objects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectFocus {
    pub cardinality: ObjectFocusCardinality,
    pub objects: Vec<ObjectDescriptor>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "primaryObjectId",
        alias = "primary_object_id"
    )]
    pub primary_object_id: Option<String>,
}

/// A semantic collection. It deliberately has no `objectId`: a query or
/// metric result is not a synthetic concrete object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectSet {
    #[serde(rename = "objectType", alias = "object_type", alias = "object_type_id")]
    pub object_type_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "sourceRef")]
    pub source_ref: Option<ObjectProjectionRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InteractionSubject {
    ObjectFocus { focus: ObjectFocus },
    ObjectSet { set: ObjectSet },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionEvent {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub intent: InteractionIntent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<InteractionSubject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "targetId")]
    pub target_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "targetRole"
    )]
    pub target_role: Option<String>,
}

/// Observation surface contract used by host routing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Responder {
    pub id: String,
    #[serde(rename = "objectType")]
    pub object_type_id: String,
    pub role: String,
    pub intents: Vec<InteractionIntent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<ObjectProjectionRef>,
    #[serde(default)]
    pub derived: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "refreshOnSelect"
    )]
    pub refresh_on_select: Option<bool>,
    pub source_anchor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionBinding {
    pub id: String,
    pub trigger: String,
    pub intents: Vec<InteractionIntent>,
    #[serde(rename = "objectType")]
    pub object_type_id: String,
    #[serde(rename = "subjectKind")]
    pub subject_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<ObjectProjectionRef>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "targetRole"
    )]
    pub target_role: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "selectionMode"
    )]
    pub selection_mode: Option<String>,
    pub priority: String,
    #[serde(default)]
    pub derived: bool,
    #[serde(default, rename = "legacyDoubleFire")]
    pub legacy_double_fire: bool,
    pub source_anchor: String,
}

/// Runtime identity descriptor. It intentionally contains no dataset row
/// payload; consumers resolve non-identity data through `source_ref`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectDescriptor {
    #[serde(rename = "objectId", alias = "object_id")]
    pub object_id: String,
    #[serde(rename = "objectType", alias = "object_type", alias = "object_type_id")]
    pub object_type_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "objectKey",
        alias = "object_key"
    )]
    pub object_key: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "entityId",
        alias = "entity_id"
    )]
    pub entity_id: Option<Value>,
    #[serde(rename = "identityValues", alias = "identity_values")]
    pub identity_values: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "sourceRef",
        alias = "source_ref"
    )]
    pub source_ref: Option<ObjectProjectionRef>,
}

/// A readable locator supplied by a dataset, table, chart, map, or world
/// projection. The locator is input metadata; only [`ObjectDescriptor::object_id`]
/// is the canonical opaque identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectLocator {
    #[serde(rename = "objectType", alias = "object_type", alias = "object_type_id")]
    pub object_type_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "objectKey",
        alias = "object_key"
    )]
    pub object_key: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "entityId",
        alias = "entity_id"
    )]
    pub entity_id: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        rename = "identityValues",
        alias = "identity_values"
    )]
    pub identity_values: BTreeMap<String, Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "sourceRef",
        alias = "source_ref"
    )]
    pub source_ref: Option<ObjectProjectionRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeObjectIndexEntry {
    pub locator: ObjectLocator,
    #[serde(rename = "objectId", alias = "object_id")]
    pub object_id: String,
}

/// Serializable thin index injected by the host. It contains descriptors and
/// locator aliases only, never owner payloads or dataset rows.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeObjectIndex {
    #[serde(default)]
    pub descriptors: BTreeMap<String, ObjectDescriptor>,
    #[serde(default)]
    pub entries: Vec<RuntimeObjectIndexEntry>,
}

impl RuntimeObjectIndex {
    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty() && self.entries.is_empty()
    }
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
    #[error("object type `{object_type_id}` identity field `{field}` is empty")]
    EmptyIdentityValue {
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
    #[error("object type `{object_type_id}` requires an identity locator")]
    MissingIdentityLocator { object_type_id: String },
    #[error(
        "object type `{object_type_id}` has {field_count} identity fields; objectKey/entityId can only resolve a single-field identity"
    )]
    AmbiguousIdentityLocator {
        object_type_id: String,
        field_count: usize,
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
        if self.identity.fields.is_empty() {
            return Err(ObjectMaterializationError::MissingIdentityLocator {
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
            object_key: None,
            entity_id: None,
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
    #[serde(default)]
    pub authoring_mode: ObjectCatalogAuthoringMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<ObjectTypeContract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<ObjectProjectionRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intents: Vec<ObjectIntent>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub index: Vec<ObjectIndexEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_assemblies: Vec<DefaultObjectAssembly>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub interaction_bindings: Vec<InteractionBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub responders: Vec<Responder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<ObjectCatalogDiagnostic>,
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

/// Process-local identity resolver. This is the only runtime entry point that
/// mints canonical object ids; callers submit locators or explicitly declared
/// dataset-row contracts and receive opaque descriptors.
#[derive(Debug, Clone, Default)]
pub struct ObjectResolver {
    object_types: BTreeMap<String, ObjectTypeContract>,
    index: RuntimeObjectIndex,
}

impl ObjectResolver {
    pub fn from_catalogs<'a>(catalogs: impl IntoIterator<Item = &'a ObjectCatalog>) -> Self {
        let mut resolver = Self::default();
        for catalog in catalogs {
            for object_type in &catalog.types {
                resolver
                    .object_types
                    .insert(object_type.id.clone(), object_type.clone());
            }
        }
        resolver
    }

    pub fn is_empty(&self) -> bool {
        self.object_types.is_empty()
    }

    pub fn object_index(&self) -> &RuntimeObjectIndex {
        &self.index
    }

    pub fn into_object_index(self) -> RuntimeObjectIndex {
        self.index
    }

    pub fn resolve_dataset_row(
        &mut self,
        object_type_id: &str,
        row: &Map<String, Value>,
    ) -> Result<ObjectDescriptor, ObjectMaterializationError> {
        let descriptor = self
            .object_type(object_type_id)?
            .materialize_dataset_row(row)?;
        self.register(
            ObjectLocator {
                object_type_id: object_type_id.to_string(),
                object_key: None,
                entity_id: None,
                identity_values: descriptor.identity_values.clone(),
                source_ref: descriptor.source_ref.clone(),
            },
            descriptor,
        )
    }

    pub fn resolve_locator(
        &mut self,
        locator: ObjectLocator,
    ) -> Result<ObjectDescriptor, ObjectMaterializationError> {
        let object_type = self.object_type(locator.object_type_id.as_str())?.clone();
        if object_type.identity.fields.is_empty() {
            return Err(ObjectMaterializationError::MissingIdentityLocator {
                object_type_id: object_type.id,
            });
        }
        let normalization = IdentityNormalization::parse(
            object_type.identity.normalization.as_deref(),
            object_type.id.as_str(),
        )?;
        let mut identity_values = BTreeMap::new();
        let mut digest_values = Vec::with_capacity(object_type.identity.fields.len());

        if locator.identity_values.is_empty() {
            if object_type.identity.fields.len() != 1 {
                return Err(ObjectMaterializationError::AmbiguousIdentityLocator {
                    object_type_id: object_type.id,
                    field_count: object_type.identity.fields.len(),
                });
            }
            let field = object_type.identity.fields[0].as_str();
            let selected = match (&locator.object_key, &locator.entity_id) {
                (Some(object_key), Some(entity_id)) => {
                    let (_, object_digest) = normalize_identity_scalar(
                        object_type.id.as_str(),
                        field,
                        object_key,
                        normalization,
                    )?;
                    let (_, entity_digest) = normalize_identity_scalar(
                        object_type.id.as_str(),
                        field,
                        entity_id,
                        normalization,
                    )?;
                    if object_digest != entity_digest {
                        return Err(ObjectMaterializationError::IdentityConflict {
                            object_type_id: object_type.id,
                            field: field.to_string(),
                            conflicting_field: "entityId".to_string(),
                        });
                    }
                    object_key
                }
                (Some(object_key), None) => object_key,
                (None, Some(entity_id)) => entity_id,
                (None, None) => {
                    return Err(ObjectMaterializationError::MissingIdentityLocator {
                        object_type_id: object_type.id,
                    })
                }
            };
            let (normalized, digest) =
                normalize_identity_scalar(object_type.id.as_str(), field, selected, normalization)?;
            identity_values.insert(field.to_string(), normalized);
            digest_values.push((field, digest));
        } else {
            for field in &object_type.identity.fields {
                let value = locator
                    .identity_values
                    .get(field)
                    .or_else(|| {
                        object_type
                            .identity
                            .aliases
                            .iter()
                            .find_map(|alias| locator.identity_values.get(alias))
                    })
                    .ok_or_else(|| ObjectMaterializationError::MissingIdentityField {
                        object_type_id: object_type.id.clone(),
                        field: field.clone(),
                        aliases: object_type.identity.aliases.clone(),
                    })?;
                let (normalized, digest) = normalize_identity_scalar(
                    object_type.id.as_str(),
                    field,
                    value,
                    normalization,
                )?;
                identity_values.insert(field.clone(), normalized);
                digest_values.push((field.as_str(), digest));
            }
        }

        let descriptor = ObjectDescriptor {
            object_id: object_id_for_identity(object_type.id.as_str(), &digest_values),
            object_type_id: object_type.id,
            object_key: locator.object_key.clone(),
            entity_id: locator.entity_id.clone(),
            identity_values,
            label: object_type.label,
            source_ref: locator.source_ref.clone().or(Some(object_type.source)),
        };
        self.register(locator, descriptor)
    }

    pub fn descriptor(&self, object_id: &str) -> Option<&ObjectDescriptor> {
        self.index.descriptors.get(object_id)
    }

    fn object_type(
        &self,
        object_type_id: &str,
    ) -> Result<&ObjectTypeContract, ObjectMaterializationError> {
        self.object_types.get(object_type_id).ok_or_else(|| {
            ObjectMaterializationError::UnknownObjectType {
                object_type_id: object_type_id.to_string(),
            }
        })
    }

    fn register(
        &mut self,
        mut locator: ObjectLocator,
        descriptor: ObjectDescriptor,
    ) -> Result<ObjectDescriptor, ObjectMaterializationError> {
        self.index
            .descriptors
            .entry(descriptor.object_id.clone())
            .or_insert_with(|| descriptor.clone());
        if locator.identity_values.is_empty() {
            locator.identity_values = descriptor.identity_values.clone();
        }
        let entry = RuntimeObjectIndexEntry {
            locator,
            object_id: descriptor.object_id.clone(),
        };
        if !self.index.entries.contains(&entry) {
            self.index.entries.push(entry);
        }
        Ok(descriptor)
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
            if value.is_empty() {
                return Err(ObjectMaterializationError::EmptyIdentityValue {
                    object_type_id: object_type_id.to_string(),
                    field: field.to_string(),
                });
            }
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
            authoring_mode: ObjectCatalogAuthoringMode::Legacy,
            types: vec![ObjectTypeContract {
                id: "zhifa.Warning".to_string(),
                intent_id: None,
                label: Some("Warning".to_string()),
                identity: ObjectIdentityContract {
                    materialization: ObjectIdentityMaterialization::DatasetRow,
                    fields: vec!["warning_id".to_string()],
                    locator: None,
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
            intents: vec![],
            index: vec![],
            default_assemblies: vec![],
            interaction_bindings: vec![],
            responders: vec![],
            diagnostics: vec![],
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
    fn legacy_catalog_decodes_without_migration() {
        let legacy = json!({
            "schema_version": "mei-object-catalog-v1",
            "id": "legacy",
            "types": [],
            "refs": [],
            "source_anchor": "domain/legacy.objects.mei"
        });
        let decoded: ObjectCatalog =
            serde_json::from_value(legacy).expect("decode pre-intent catalog");
        assert_eq!(decoded.authoring_mode, ObjectCatalogAuthoringMode::Legacy);
        assert!(decoded.intents.is_empty());
        assert!(decoded.index.is_empty());
        assert!(decoded.default_assemblies.is_empty());
        assert!(decoded.interaction_bindings.is_empty());
        assert!(decoded.responders.is_empty());
    }

    #[test]
    fn object_intent_index_and_default_assembly_serialize_stably_without_owner_payload() {
        let source_anchor = "domain/alerts.objects.mei";
        let source = ObjectProjectionRef {
            role: "source".to_string(),
            kind: "dataset_ref".to_string(),
            id: "alerts".to_string(),
            source_anchor: source_anchor.to_string(),
        };
        let identity = ObjectProjectionRef {
            role: "identity".to_string(),
            kind: "field_ref".to_string(),
            id: "alert_id".to_string(),
            source_anchor: source_anchor.to_string(),
        };
        let recipe = ObjectProjectionRef {
            role: "recipe".to_string(),
            kind: "stock_ref".to_string(),
            id: "alert".to_string(),
            source_anchor: source_anchor.to_string(),
        };
        let mut slots = BTreeMap::new();
        slots.insert(
            "summary".to_string(),
            ObjectProjectionRef {
                role: "slot:summary".to_string(),
                kind: "field_ref".to_string(),
                id: "title".to_string(),
                source_anchor: source_anchor.to_string(),
            },
        );
        let intent = ObjectIntent {
            intent_id: "intent_stable".to_string(),
            object_type_id: "ops.Alert".to_string(),
            source: source.clone(),
            identity: ObjectIdentityContract {
                materialization: ObjectIdentityMaterialization::DatasetRow,
                fields: vec!["alert_id".to_string()],
                locator: Some(identity.clone()),
                aliases: Vec::new(),
                normalization: None,
            },
            recipe: recipe.clone(),
            slots: slots.clone(),
            relations: BTreeMap::new(),
            override_props: Some(json!({"density": "compact"})),
            owner_hints: vec![source.clone(), identity.clone(), recipe.clone()],
            source_anchor: source_anchor.to_string(),
        };
        let index = ObjectIndexEntry {
            kind: OBJECT_INDEX_ENTRY_KIND.to_string(),
            key: "ops.Alert::dataset_ref:alerts::field_ref:alert_id".to_string(),
            intent_id: intent.intent_id.clone(),
            object_type_id: intent.object_type_id.clone(),
            source,
            identity,
            recipe: recipe.clone(),
            owner_hints: intent.owner_hints.clone(),
            source_anchor: source_anchor.to_string(),
        };
        let assembly = DefaultObjectAssembly {
            kind: DEFAULT_OBJECT_ASSEMBLY_KIND.to_string(),
            id: "assembly_stable".to_string(),
            intent_id: intent.intent_id.clone(),
            recipe,
            recipe_contract: None,
            slots,
            relations: BTreeMap::new(),
            override_props: intent.override_props.clone(),
            effective_override: None,
            override_sources: BTreeMap::new(),
            projections: Vec::new(),
            source_anchor: source_anchor.to_string(),
        };

        let first = serde_json::to_string(&(intent, index, assembly)).expect("serialize contracts");
        let second = serde_json::to_string(
            &serde_json::from_str::<(ObjectIntent, ObjectIndexEntry, DefaultObjectAssembly)>(
                &first,
            )
            .expect("decode contracts"),
        )
        .expect("serialize contracts again");
        assert_eq!(first, second);
        assert!(!first.contains("\"payload\""));
        assert!(!first.contains("\"rows\""));
    }

    #[test]
    fn interaction_protocol_has_five_stable_intents_and_distinct_focus_and_set_subjects() {
        let intents = [
            InteractionIntent::Select,
            InteractionIntent::OpenProjection,
            InteractionIntent::ExplainMetric,
            InteractionIntent::FilterQuery,
            InteractionIntent::FocusViewpoint,
        ];
        assert_eq!(
            serde_json::to_value(intents).expect("serialize intents"),
            json!([
                "select",
                "open_projection",
                "explain_metric",
                "filter_query",
                "focus_viewpoint"
            ])
        );

        let focus = InteractionSubject::ObjectFocus {
            focus: ObjectFocus {
                cardinality: ObjectFocusCardinality::Single,
                objects: vec![ObjectDescriptor {
                    object_id: "obj_canonical".to_string(),
                    object_type_id: "ops.Alert".to_string(),
                    object_key: None,
                    entity_id: None,
                    identity_values: BTreeMap::from([("alert_id".to_string(), json!("A-1"))]),
                    label: None,
                    source_ref: None,
                }],
                primary_object_id: Some("obj_canonical".to_string()),
            },
        };
        let set = InteractionSubject::ObjectSet {
            set: ObjectSet {
                object_type_id: "ops.Alert".to_string(),
                query: Some(json!({"severity": "high"})),
                metric: Some("open_alerts".to_string()),
                source_ref: None,
            },
        };
        let focus_value = serde_json::to_value(focus).expect("focus serde");
        let event = InteractionEvent {
            schema_version: INTERACTION_PROTOCOL_SCHEMA_VERSION.to_string(),
            intent: InteractionIntent::ExplainMetric,
            subject: Some(set),
            source: Some("metric-card".to_string()),
            target_id: None,
            target_role: Some("chart".to_string()),
        };
        let set_value = serde_json::to_value(event).expect("event serde");
        assert_eq!(focus_value["kind"], "object_focus");
        assert_eq!(
            set_value["schemaVersion"],
            INTERACTION_PROTOCOL_SCHEMA_VERSION
        );
        assert_eq!(set_value["intent"], "explain_metric");
        assert_eq!(set_value["subject"]["kind"], "object_set");
        assert!(set_value.pointer("/subject/set/objectId").is_none());
        assert!(set_value.pointer("/subject/set/object_id").is_none());
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
        assert_eq!(descriptor["sourceRef"]["id"], "warning_rows");
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

        let empty = object_type
            .materialize_dataset_row(json!({"预警ID": "  "}).as_object().expect("row"))
            .expect_err("empty identity");
        assert!(matches!(
            empty,
            ObjectMaterializationError::EmptyIdentityValue { .. }
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
            authoring_mode: ObjectCatalogAuthoringMode::Legacy,
            types: vec![warning_type()],
            refs: Vec::new(),
            intents: Vec::new(),
            index: Vec::new(),
            default_assemblies: Vec::new(),
            interaction_bindings: Vec::new(),
            responders: Vec::new(),
            diagnostics: Vec::new(),
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

    #[test]
    fn resolver_unifies_dataset_object_key_and_entity_id_identity() {
        let catalog = warning_catalog();
        let mut resolver = ObjectResolver::from_catalogs([&catalog]);
        let row = json!({"预警ID": " W-001 ", "title": "row"});
        let from_row = resolver
            .resolve_dataset_row("zhifa.Warning", row.as_object().expect("row"))
            .expect("dataset identity");
        let from_object_key = resolver
            .resolve_locator(ObjectLocator {
                object_type_id: "zhifa.Warning".to_string(),
                object_key: Some(json!("W-001")),
                entity_id: None,
                identity_values: BTreeMap::new(),
                source_ref: None,
            })
            .expect("objectKey identity");
        let from_entity = resolver
            .resolve_locator(ObjectLocator {
                object_type_id: "zhifa.Warning".to_string(),
                object_key: None,
                entity_id: Some(json!("W-001")),
                identity_values: BTreeMap::new(),
                source_ref: None,
            })
            .expect("entityId identity");

        assert_eq!(from_row.object_id, from_object_key.object_id);
        assert_eq!(from_row.object_id, from_entity.object_id);
        assert_eq!(resolver.object_index().descriptors.len(), 1);
        assert_eq!(resolver.object_index().entries.len(), 3);
    }

    #[test]
    fn resolver_matches_repository_runtime_index_golden() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/object-identity/runtime-index.json"
        ))
        .expect("runtime identity fixture");
        let catalog = warning_catalog();
        let mut resolver = ObjectResolver::from_catalogs([&catalog]);
        let descriptor = resolver
            .resolve_locator(ObjectLocator {
                object_type_id: fixture["objectType"]
                    .as_str()
                    .expect("fixture objectType")
                    .to_string(),
                object_key: Some(fixture["identityValue"].clone()),
                entity_id: Some(fixture["identityValue"].clone()),
                identity_values: BTreeMap::new(),
                source_ref: None,
            })
            .expect("golden locator");
        assert_eq!(
            descriptor.object_id,
            fixture["objectId"].as_str().expect("fixture objectId")
        );
    }

    #[test]
    fn resolver_type_namespace_prevents_cross_type_collisions() {
        let mut second = warning_type();
        second.id = "ops.Warning".to_string();
        let mut catalog = warning_catalog();
        catalog.types.push(second);
        let mut resolver = ObjectResolver::from_catalogs([&catalog]);
        let first = resolver
            .resolve_locator(ObjectLocator {
                object_type_id: "zhifa.Warning".to_string(),
                object_key: Some(json!("W-001")),
                entity_id: None,
                identity_values: BTreeMap::new(),
                source_ref: None,
            })
            .expect("first type");
        let second = resolver
            .resolve_locator(ObjectLocator {
                object_type_id: "ops.Warning".to_string(),
                object_key: Some(json!("W-001")),
                entity_id: None,
                identity_values: BTreeMap::new(),
                source_ref: None,
            })
            .expect("second type");
        assert_ne!(first.object_id, second.object_id);
    }

    #[test]
    fn resolver_rejects_missing_empty_and_conflicting_locators() {
        let catalog = warning_catalog();
        let mut resolver = ObjectResolver::from_catalogs([&catalog]);
        let missing = resolver
            .resolve_locator(ObjectLocator {
                object_type_id: "zhifa.Warning".to_string(),
                object_key: None,
                entity_id: None,
                identity_values: BTreeMap::new(),
                source_ref: None,
            })
            .expect_err("missing locator");
        assert!(matches!(
            missing,
            ObjectMaterializationError::MissingIdentityLocator { .. }
        ));

        let empty = resolver
            .resolve_locator(ObjectLocator {
                object_type_id: "zhifa.Warning".to_string(),
                object_key: Some(json!(" ")),
                entity_id: None,
                identity_values: BTreeMap::new(),
                source_ref: None,
            })
            .expect_err("empty locator");
        assert!(matches!(
            empty,
            ObjectMaterializationError::EmptyIdentityValue { .. }
        ));

        let conflict = resolver
            .resolve_locator(ObjectLocator {
                object_type_id: "zhifa.Warning".to_string(),
                object_key: Some(json!("W-001")),
                entity_id: Some(json!("W-002")),
                identity_values: BTreeMap::new(),
                source_ref: None,
            })
            .expect_err("conflicting locator aliases");
        assert!(matches!(
            conflict,
            ObjectMaterializationError::IdentityConflict { .. }
        ));
    }

    fn warning_catalog() -> ObjectCatalog {
        ObjectCatalog {
            schema_version: OBJECT_CATALOG_SCHEMA_VERSION.to_string(),
            id: "warning_objects".to_string(),
            authoring_mode: ObjectCatalogAuthoringMode::AuthorIntent,
            types: vec![warning_type()],
            refs: Vec::new(),
            intents: Vec::new(),
            index: Vec::new(),
            default_assemblies: Vec::new(),
            interaction_bindings: Vec::new(),
            responders: Vec::new(),
            diagnostics: Vec::new(),
            source_anchor: "domain/warnings.objects.mei".to_string(),
        }
    }

    fn warning_type() -> ObjectTypeContract {
        ObjectTypeContract {
            id: "zhifa.Warning".to_string(),
            intent_id: None,
            label: Some("Warning".to_string()),
            identity: ObjectIdentityContract {
                materialization: ObjectIdentityMaterialization::DatasetRow,
                fields: vec!["预警ID".to_string()],
                locator: None,
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
