use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const UPLOAD_REGISTRY_REL_PATH: &str = "upload/.mei-upload-registry.json";
const LEGACY_UPLOAD_REGISTRY_REL_PATH: &str = "assets/upload/.mei-upload-registry.json";
const LEGACY_UPLOAD_PREFIX: &str = "upload/";

fn upload_registry_candidate_paths(app_root: &Path) -> [PathBuf; 2] {
    [
        app_root.join(LEGACY_UPLOAD_REGISTRY_REL_PATH),
        app_root.join(UPLOAD_REGISTRY_REL_PATH),
    ]
}

fn resolve_upload_registry_path(app_root: &Path) -> PathBuf {
    upload_registry_candidate_paths(app_root)
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| app_root.join(LEGACY_UPLOAD_REGISTRY_REL_PATH))
}

/// Map legacy app-relative `upload/...` onto v2 `assets/upload/...` when the former is absent.
pub fn resolve_legacy_app_upload_path(app_root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = normalize_path(rel);
    let tail = rel.strip_prefix(LEGACY_UPLOAD_PREFIX)?;
    if tail.is_empty() {
        return None;
    }
    let under_assets = app_root.join("assets/upload").join(tail);
    under_assets.is_file().then_some(under_assets)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVersionedUploadFile {
    pub base_name: String,
    pub version: String,
    pub ext: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UploadVersionRecord {
    pub version: String,
    pub physical_path: String,
    #[serde(default)]
    pub content_hash: Option<String>,
    #[serde(default)]
    pub uploaded_at: Option<String>,
    #[serde(default)]
    pub uploaded_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UploadAliasRecord {
    pub alias_path: String,
    pub base_name: String,
    pub ext: String,
    #[serde(default)]
    pub current_version: Option<String>,
    #[serde(default)]
    pub current_physical_path: String,
    #[serde(default)]
    pub versions: Vec<UploadVersionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UploadRegistry {
    #[serde(default)]
    pub aliases: BTreeMap<String, UploadAliasRecord>,
}

pub fn parse_versioned_upload_file_name(file_name: &str) -> Option<ParsedVersionedUploadFile> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let dot = trimmed.rfind('.')?;
    let ext = trimmed[(dot + 1)..].to_ascii_lowercase();
    if !matches!(ext.as_str(), "xlsx" | "xls" | "csv") {
        return None;
    }
    let stem = &trimmed[..dot];
    let split = stem.rfind('.')?;
    let base_name = stem[..split].trim();
    let version = stem[(split + 1)..].trim().to_ascii_uppercase();
    if base_name.is_empty() || !is_valid_version_token(version.as_str()) {
        return None;
    }
    Some(ParsedVersionedUploadFile {
        base_name: base_name.to_string(),
        version,
        ext,
    })
}

pub fn compare_version_tokens(left: &str, right: &str) -> Option<Ordering> {
    let left = normalize_version_token(left)?;
    let right = normalize_version_token(right)?;
    Some(left.cmp(&right))
}

pub fn read_upload_registry(app_root: &Path) -> Option<UploadRegistry> {
    for path in upload_registry_candidate_paths(app_root) {
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        if let Ok(registry) = serde_json::from_str::<UploadRegistry>(&raw) {
            return Some(registry);
        }
    }
    None
}

pub fn write_upload_registry(app_root: &Path, registry: &UploadRegistry) -> std::io::Result<()> {
    let path = resolve_upload_registry_path(app_root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let raw = serde_json::to_string_pretty(registry)
        .map_err(|error| io::Error::other(error.to_string()))?;
    fs::write(path, raw)
}

pub fn register_upload_version(
    app_root: &Path,
    alias_path: &str,
    versioned_file_name: &str,
    physical_path: &str,
    content_hash: Option<String>,
    uploaded_at: Option<String>,
    uploaded_by: Option<String>,
) -> io::Result<UploadAliasRecord> {
    let parsed = parse_versioned_upload_file_name(versioned_file_name).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "invalid versioned file name")
    })?;
    let normalized_alias = normalize_path(alias_path);
    if normalized_alias.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "alias_path cannot be empty",
        ));
    }
    let normalized_physical = normalize_path(physical_path);
    if normalized_physical.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "physical_path cannot be empty",
        ));
    }
    let mut registry = read_upload_registry(app_root).unwrap_or_default();
    let alias_keys = alias_lookup_keys_for_registration(
        normalized_alias.as_str(),
        parsed.base_name.as_str(),
        parsed.ext.as_str(),
    );
    let mut snapshot = None;
    for alias_key in alias_keys {
        let entry = registry
            .aliases
            .entry(alias_key.clone())
            .or_insert_with(|| UploadAliasRecord {
                alias_path: alias_key.clone(),
                base_name: parsed.base_name.clone(),
                ext: alias_ext(alias_key.as_str()),
                current_version: None,
                current_physical_path: String::new(),
                versions: Vec::new(),
            });
        entry.alias_path = alias_key.clone();
        entry.base_name = parsed.base_name.clone();
        entry.ext = alias_ext(alias_key.as_str());
        upsert_version_record(
            entry,
            parsed.version.as_str(),
            normalized_physical.as_str(),
            content_hash.clone(),
            uploaded_at.clone(),
            uploaded_by.clone(),
        );
        if alias_key == normalized_alias {
            snapshot = Some(entry.clone());
        }
    }
    let snapshot = snapshot.unwrap_or_else(|| UploadAliasRecord {
        alias_path: normalized_alias,
        base_name: parsed.base_name,
        ext: parsed.ext,
        current_version: None,
        current_physical_path: String::new(),
        versions: Vec::new(),
    });
    write_upload_registry(app_root, &registry)?;
    Ok(snapshot)
}

pub fn resolve_versioned_source_path(app_root: &Path, source_path: &str) -> PathBuf {
    resolve_versioned_source_meta(app_root, source_path).absolute_path
}

pub fn resolve_versioned_source_identifier(app_root: &Path, source_path: &str) -> String {
    resolve_versioned_source_meta(app_root, source_path).resolved_identifier
}

#[derive(Debug, Clone)]
struct ResolvedVersionedSource {
    absolute_path: PathBuf,
    resolved_identifier: String,
}

fn resolve_versioned_source_meta(app_root: &Path, source_path: &str) -> ResolvedVersionedSource {
    let source_path = source_path.trim();
    if source_path.is_empty() {
        return ResolvedVersionedSource {
            absolute_path: app_root.to_path_buf(),
            resolved_identifier: String::new(),
        };
    }
    let path = Path::new(source_path);
    if path.is_absolute() {
        return ResolvedVersionedSource {
            absolute_path: path.to_path_buf(),
            resolved_identifier: normalize_path(source_path),
        };
    }
    let normalized = normalize_path(source_path);
    let registry = read_upload_registry(app_root);
    let current = registry
        .and_then(|value| resolve_registry_physical_path(&value, normalized.as_str()))
        .unwrap_or_else(|| normalized.clone());
    let absolute_path = if Path::new(&current).is_absolute() {
        PathBuf::from(&current)
    } else {
        app_root.join(&current)
    };
    let absolute_path = if absolute_path.is_file() {
        absolute_path
    } else if let Some(fallback) = resolve_legacy_app_upload_path(app_root, current.as_str()) {
        fallback
    } else if let Some(fallback) = resolve_legacy_app_upload_path(app_root, normalized.as_str()) {
        fallback
    } else {
        absolute_path
    };
    ResolvedVersionedSource {
        absolute_path,
        resolved_identifier: current,
    }
}

fn normalize_path(raw: &str) -> String {
    raw.trim().replace('\\', "/")
}

fn resolve_registry_physical_path(registry: &UploadRegistry, alias_path: &str) -> Option<String> {
    let keys = alias_lookup_keys(alias_path);
    let best = keys
        .into_iter()
        .filter_map(|key| registry.aliases.get(&key))
        .filter(|entry| !entry.current_physical_path.trim().is_empty())
        .max_by(|left, right| compare_registry_entries(left, right))?;
    Some(normalize_path(best.current_physical_path.as_str()))
}

fn compare_registry_entries(left: &UploadAliasRecord, right: &UploadAliasRecord) -> Ordering {
    match (
        left.current_version.as_deref(),
        right.current_version.as_deref(),
    ) {
        (Some(l), Some(r)) => compare_version_tokens(l, r).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn upsert_version_record(
    entry: &mut UploadAliasRecord,
    version: &str,
    physical_path: &str,
    content_hash: Option<String>,
    uploaded_at: Option<String>,
    uploaded_by: Option<String>,
) {
    if let Some(existing) = entry
        .versions
        .iter_mut()
        .find(|item| item.version.eq_ignore_ascii_case(version))
    {
        existing.physical_path = physical_path.to_string();
        existing.content_hash = content_hash;
        existing.uploaded_at = uploaded_at;
        existing.uploaded_by = uploaded_by;
    } else {
        entry.versions.push(UploadVersionRecord {
            version: version.to_string(),
            physical_path: physical_path.to_string(),
            content_hash,
            uploaded_at,
            uploaded_by,
        });
    }
    entry.versions.sort_by(|left, right| {
        compare_version_tokens(left.version.as_str(), right.version.as_str())
            .unwrap_or(Ordering::Equal)
    });
    if let Some(latest) = entry.versions.last().cloned() {
        entry.current_version = Some(latest.version);
        entry.current_physical_path = latest.physical_path;
    }
}

fn alias_lookup_keys_for_registration(alias_path: &str, base_name: &str, ext: &str) -> Vec<String> {
    if !is_excel_ext(ext) {
        return vec![alias_path.to_string()];
    }
    let base_alias = alias_without_ext(alias_path).unwrap_or_else(|| base_name.to_string());
    vec![format!("{base_alias}.xlsx"), format!("{base_alias}.xls")]
}

fn alias_lookup_keys(alias_path: &str) -> Vec<String> {
    let normalized = normalize_path(alias_path);
    let ext = alias_ext(normalized.as_str());
    if !is_excel_ext(ext.as_str()) {
        return vec![normalized];
    }
    let base_alias = alias_without_ext(normalized.as_str()).unwrap_or(normalized.clone());
    vec![format!("{base_alias}.xlsx"), format!("{base_alias}.xls")]
}

fn alias_without_ext(alias_path: &str) -> Option<String> {
    let dot = alias_path.rfind('.')?;
    Some(alias_path[..dot].to_string())
}

fn alias_ext(alias_path: &str) -> String {
    alias_path
        .rsplit('.')
        .next()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_excel_ext(ext: &str) -> bool {
    matches!(ext, "xlsx" | "xls")
}

fn normalize_version_token(token: &str) -> Option<(String, String)> {
    let normalized = token.trim().to_ascii_uppercase();
    if !is_valid_version_token(normalized.as_str()) {
        return None;
    }
    let date = normalized[..8].to_string();
    let suffix = normalized[8..].to_string();
    Some((date, suffix))
}

fn is_valid_version_token(token: &str) -> bool {
    if token.len() < 8 {
        return false;
    }
    let (date, suffix) = token.split_at(8);
    date.chars().all(|ch| ch.is_ascii_digit())
        && suffix
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() && ch.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::{
        compare_version_tokens, parse_versioned_upload_file_name, read_upload_registry,
        register_upload_version, resolve_versioned_source_identifier,
        resolve_versioned_source_path, write_upload_registry, UploadAliasRecord, UploadRegistry,
        UploadVersionRecord,
    };
    use std::cmp::Ordering;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn parse_versioned_upload_file_name_accepts_supported_patterns() {
        let parsed =
            parse_versioned_upload_file_name("11.预警清单.20260527A.xlsx").expect("versioned name");
        assert_eq!(parsed.base_name, "11.预警清单");
        assert_eq!(parsed.version, "20260527A");
        assert_eq!(parsed.ext, "xlsx");
    }

    #[test]
    fn parse_versioned_upload_file_name_rejects_invalid_tokens() {
        assert!(parse_versioned_upload_file_name("foo.2026052.xlsx").is_none());
        assert!(parse_versioned_upload_file_name("foo.20260527a.xlsx").is_some());
        assert!(parse_versioned_upload_file_name("foo.20260527-.xlsx").is_none());
        assert!(parse_versioned_upload_file_name("foo.20260527.txt").is_none());
    }

    #[test]
    fn compare_version_tokens_orders_date_and_suffix() {
        assert_eq!(
            compare_version_tokens("20260527", "20260527A"),
            Some(Ordering::Less)
        );
        assert_eq!(
            compare_version_tokens("20260527B", "20260527A"),
            Some(Ordering::Greater)
        );
        assert_eq!(
            compare_version_tokens("20260528", "20260527Z"),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn resolve_versioned_source_path_prefers_registry_current_physical_path() {
        let root = std::env::temp_dir().join(format!("mei-upload-registry-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("upload/.versions")).expect("create version dir");
        let mut aliases = BTreeMap::new();
        aliases.insert(
            "upload/11.预警清单.xlsx".to_string(),
            UploadAliasRecord {
                alias_path: "upload/11.预警清单.xlsx".to_string(),
                base_name: "11.预警清单".to_string(),
                ext: "xlsx".to_string(),
                current_version: Some("20260527A".to_string()),
                current_physical_path: "upload/.versions/11.预警清单.20260527A.xlsx".to_string(),
                versions: vec![UploadVersionRecord {
                    version: "20260527A".to_string(),
                    physical_path: "upload/.versions/11.预警清单.20260527A.xlsx".to_string(),
                    content_hash: None,
                    uploaded_at: None,
                    uploaded_by: None,
                }],
            },
        );
        write_upload_registry(&root, &UploadRegistry { aliases }).expect("write registry");
        let loaded = read_upload_registry(&root).expect("load registry");
        assert!(loaded.aliases.contains_key("upload/11.预警清单.xlsx"));
        let resolved = resolve_versioned_source_path(&root, "upload/11.预警清单.xlsx");
        assert!(resolved.ends_with("upload/.versions/11.预警清单.20260527A.xlsx"));
        let id = resolve_versioned_source_identifier(&root, "upload/11.预警清单.xlsx");
        assert_eq!(id, "upload/.versions/11.预警清单.20260527A.xlsx");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn register_upload_version_syncs_excel_alias_variants_and_prefers_latest() {
        let root =
            std::env::temp_dir().join(format!("mei-upload-registry-excel-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("upload/.versions")).expect("create version dir");
        register_upload_version(
            &root,
            "upload/11.预警清单.xlsx",
            "11.预警清单.20260527A.xls",
            "upload/.versions/11.预警清单.20260527A.xls",
            None,
            None,
            None,
        )
        .expect("register xls");
        register_upload_version(
            &root,
            "upload/11.预警清单.xlsx",
            "11.预警清单.20260527B.xlsx",
            "upload/.versions/11.预警清单.20260527B.xlsx",
            None,
            None,
            None,
        )
        .expect("register xlsx");
        let loaded = read_upload_registry(&root).expect("load registry");
        assert!(loaded.aliases.contains_key("upload/11.预警清单.xlsx"));
        assert!(loaded.aliases.contains_key("upload/11.预警清单.xls"));
        let resolved_xlsx = resolve_versioned_source_identifier(&root, "upload/11.预警清单.xlsx");
        let resolved_xls = resolve_versioned_source_identifier(&root, "upload/11.预警清单.xls");
        assert_eq!(resolved_xlsx, "upload/.versions/11.预警清单.20260527B.xlsx");
        assert_eq!(resolved_xls, "upload/.versions/11.预警清单.20260527B.xlsx");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn resolve_versioned_source_path_falls_back_to_assets_upload() {
        let root = std::env::temp_dir().join(format!(
            "mei-upload-legacy-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("assets/upload")).expect("mkdir assets upload");
        fs::write(
            root.join("assets/upload/8.行政处罚结果清单.xlsx"),
            b"xlsx",
        )
        .expect("write xlsx");
        let resolved =
            resolve_versioned_source_path(&root, "upload/8.行政处罚结果清单.xlsx");
        assert_eq!(
            resolved,
            root.join("assets/upload/8.行政处罚结果清单.xlsx")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
