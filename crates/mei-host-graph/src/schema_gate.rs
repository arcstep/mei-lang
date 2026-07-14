//! Schema version gate for cached layer documents (Gate C).

use serde_json::Value;

/// Return true when JSON bytes declare `schema_version == expected`.
pub fn layer_bytes_match_schema(bytes: &[u8], expected: &str) -> bool {
    let Ok(value) = serde_json::from_slice::<Value>(bytes) else {
        return false;
    };
    value
        .get("schema_version")
        .and_then(|v| v.as_str())
        .is_some_and(|got| got == expected)
}

/// After deserialize, reject documents whose schema_version field mismatches.
pub fn document_schema_ok(got: &str, expected: &str) -> bool {
    got.trim() == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_bytes_refuse_wrong_schema() {
        let bytes = br#"{"schema_version":"old-v0","x":1}"#;
        assert!(!layer_bytes_match_schema(bytes, "shell-v1"));
        let ok = br#"{"schema_version":"shell-v1"}"#;
        assert!(layer_bytes_match_schema(ok, "shell-v1"));
    }

    #[test]
    fn document_schema_ok_trims_and_compares() {
        assert!(document_schema_ok(" shell-v1 ", "shell-v1"));
        assert!(!document_schema_ok("shell-v0", "shell-v1"));
    }

    /// Contract mirror of server `PREBUILD_COMPILE_INDEX_SCHEMA_V9` refuse (inventory locks source).
    #[test]
    fn prebuild_compile_index_schema_contract_v9() {
        const CURRENT: &str = "mei-prebuild-compile-index-v9";
        assert!(document_schema_ok(CURRENT, CURRENT));
        assert!(!document_schema_ok(
            "mei-prebuild-compile-index-v8",
            CURRENT
        ));
        assert!(!layer_bytes_match_schema(
            br#"{"schema_version":"mei-prebuild-compile-index-v8","entries":[]}"#,
            CURRENT
        ));
        assert!(layer_bytes_match_schema(
            br#"{"schema_version":"mei-prebuild-compile-index-v9","entries":[]}"#,
            CURRENT
        ));
    }
}
