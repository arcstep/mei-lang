//! Shared presentation_map schema gate (producer + consumer).

use serde_json::Value;

/// Canonical presentation_map schema id written by assemble and required by consumers.
pub const PRESENTATION_MAP_SCHEMA_VERSION: &str = "mei-presentation-map-v1";

/// Read schema version from either camelCase or snake_case wire fields.
pub fn presentation_map_schema_version(value: &Value) -> Option<&str> {
    value
        .get("schemaVersion")
        .or_else(|| value.get("schema_version"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Empty / null maps are treated as absent (not a schema error).
pub fn presentation_map_is_absent(value: &Value) -> bool {
    value.is_null() || value.as_object().is_some_and(|obj| obj.is_empty())
}

/// Accept a presentation_map document for consumer paths.
///
/// Returns `Ok(None)` when the map is absent; `Ok(Some(value))` when schema matches;
/// `Err` when a non-empty document declares a missing or unsupported schema.
pub fn accept_presentation_map(value: &Value) -> Result<Option<&Value>, String> {
    if presentation_map_is_absent(value) {
        return Ok(None);
    }
    match presentation_map_schema_version(value) {
        Some(got) if got == PRESENTATION_MAP_SCHEMA_VERSION => Ok(Some(value)),
        Some(got) => Err(format!(
            "unsupported presentation_map schemaVersion={got}; expected {PRESENTATION_MAP_SCHEMA_VERSION}"
        )),
        None => Err(format!(
            "presentation_map missing schemaVersion; expected {PRESENTATION_MAP_SCHEMA_VERSION}"
        )),
    }
}

/// True when value is absent or matches the canonical schema.
pub fn presentation_map_schema_ok(value: &Value) -> bool {
    accept_presentation_map(value).is_ok()
}
