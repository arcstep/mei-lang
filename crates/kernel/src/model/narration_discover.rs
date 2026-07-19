use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mei_syntax::{parse_deck_source_file, parse_narration_track_file};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use super::compile_out::CompiledApp;
use super::diagnostic::{Diagnostic, Severity};
use super::narration_abi::{NarrationCatalog, NarrationCue, NarrationTiming, NarrationTrack};

pub const NARRATION_TARGET_UNKNOWN: &str = "narration_target_unknown";
pub const NARRATION_TARGET_PRIVATE: &str = "narration_target_private";
pub const NARRATION_TRACK_ID_DUPLICATE: &str = "narration_track_id_duplicate";
pub const NARRATION_DEFAULT_TRACK_AMBIGUOUS: &str = "narration_default_track_ambiguous";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NarrationTrackRegistry {
    pub tracks: Vec<NarrationTrackRegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NarrationTrackRegistryEntry {
    pub track_id: String,
    pub source_anchor: String,
    pub default_for: Vec<String>,
    pub digest: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NarrationTargetCatalog {
    /// target_ref -> public definition source anchor
    pub targets: BTreeMap<String, String>,
}

impl NarrationTargetCatalog {
    pub fn contains(&self, target_ref: &str) -> bool {
        self.targets.contains_key(target_ref)
    }

    pub fn insert(&mut self, target_ref: impl Into<String>, anchor: impl Into<String>) {
        self.targets.insert(target_ref.into(), anchor.into());
    }

    pub fn from_compiled(compiled: &CompiledApp, app_root: &Path) -> Self {
        let mut out = Self::default();
        for stage in &compiled.stage_registry.stages {
            out.insert(format!("stage:{}", stage.id), stage.source_anchor.clone());
        }
        for program in compiled.stage_programs.programs.values() {
            for unit in &program.units {
                if let Some(slide_id) = unit.slide_id.as_deref() {
                    out.insert(
                        format!("stage:{}/slide:{slide_id}", program.stage_id),
                        unit.source_anchor.clone(),
                    );
                }
            }
        }
        collect_deck_targets(app_root, &mut out);
        collect_scene_targets(compiled, app_root, &mut out);
        collect_t2_targets(compiled, &mut out);
        collect_admin_targets(app_root, compiled.app_id.as_str(), &mut out);
        out
    }

    /// Build the Admin Page public target catalog from Admin Registry + PageProgram roots.
    pub fn from_admin(app_root: &Path, app_id: &str) -> Self {
        let mut out = Self::default();
        collect_admin_targets(app_root, app_id, &mut out);
        out
    }
}

pub fn discover_narration_track_paths(app_root: &Path) -> Vec<PathBuf> {
    let root = app_root.join("src/narration");
    if !root.is_dir() {
        return Vec::new();
    }
    let mut paths: Vec<_> = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".track.mdx"))
        })
        .collect();
    paths.sort();
    paths
}

pub fn discover_narration_catalog(
    app_root: &Path,
    app_id: &str,
    targets: &NarrationTargetCatalog,
) -> (NarrationTrackRegistry, NarrationCatalog, Vec<Diagnostic>) {
    let mut registry = NarrationTrackRegistry::default();
    let mut catalog = NarrationCatalog {
        catalog_id: format!("narration:{app_id}"),
        app_id: app_id.to_string(),
        source_anchor: Some("src/narration".to_string()),
        ..NarrationCatalog::default()
    };
    let mut diagnostics = Vec::new();
    let mut ids = BTreeSet::new();
    let app_prefix = format!("{}/", app_root.display().to_string().replace('\\', "/"));
    for path in discover_narration_track_paths(app_root) {
        let parsed = match parse_narration_track_file(&path) {
            Ok(parsed) => parsed,
            Err(error) => {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: error.code,
                    message: error.message,
                    source_path: Some(format!("{}:{}", relative_path(app_root, &path), error.line)),
                });
                continue;
            }
        };
        let track_id = parsed.frontmatter.id.clone();
        if !ids.insert(track_id.clone()) {
            diagnostics.push(Diagnostic {
                severity: Severity::Error,
                code: NARRATION_TRACK_ID_DUPLICATE.to_string(),
                message: format!("duplicate narration track id `{track_id}`"),
                source_path: Some(relative_path(app_root, &path)),
            });
            continue;
        }
        let source_anchor = relative_path(app_root, &path);
        let cues = parsed
            .cues
            .into_iter()
            .map(|cue| {
                let cue_anchor = cue
                    .source_anchor
                    .strip_prefix(&app_prefix)
                    .unwrap_or(cue.source_anchor.as_str());
                validate_target(
                    cue.target_ref.as_str(),
                    cue_anchor,
                    targets,
                    &mut diagnostics,
                );
                NarrationCue {
                    id: cue.id,
                    target_ref: cue.target_ref,
                    body: cue.body,
                    caption: cue.caption,
                    speaker_notes: cue.speaker_notes,
                    actions: cue.actions,
                    timing: cue.timing.map(|timing| match timing {
                        mei_syntax::NarrationTiming::Milliseconds(value) => {
                            NarrationTiming::Milliseconds(value)
                        }
                        mei_syntax::NarrationTiming::Manual => NarrationTiming::Manual,
                    }),
                    source_anchor: cue_anchor.to_string(),
                }
            })
            .collect();
        let mut track = NarrationTrack {
            id: track_id.clone(),
            title: parsed.frontmatter.title,
            scope: parsed.frontmatter.scope,
            entry: parsed.frontmatter.entry,
            default_for: parsed.frontmatter.default_for,
            summary: parsed.frontmatter.summary,
            cues,
            default_timing_ms: parsed.frontmatter.default_timing_ms,
            voice: parsed.frontmatter.voice,
            source_anchor: source_anchor.clone(),
            digest: String::new(),
        };
        track.digest = digest_serializable(&track);
        for entry in &track.default_for {
            if let Some(previous) = catalog
                .default_track_by_entry
                .insert(entry.clone(), track.id.clone())
            {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    code: NARRATION_DEFAULT_TRACK_AMBIGUOUS.to_string(),
                    message: format!(
                        "entry `{entry}` has multiple default tracks: `{previous}` and `{}`",
                        track.id
                    ),
                    source_path: Some(source_anchor.clone()),
                });
            }
        }
        registry.tracks.push(NarrationTrackRegistryEntry {
            track_id,
            source_anchor,
            default_for: track.default_for.clone(),
            digest: track.digest.clone(),
        });
        catalog.tracks.push(track);
    }
    catalog.source_digest = digest_serializable(&catalog.tracks);
    (registry, catalog, diagnostics)
}

pub fn apply_app_narration_catalog(compiled: &mut CompiledApp, app_root: &Path) {
    let targets = NarrationTargetCatalog::from_compiled(compiled, app_root);
    let (_, catalog, diagnostics) =
        discover_narration_catalog(app_root, compiled.app_id.as_str(), &targets);
    compiled.narration_catalogs.clear();
    if !catalog.tracks.is_empty() {
        let key = catalog.catalog_id.clone();
        let digest = super::abi_project::compute_narration_digest(&BTreeMap::from([(
            key.clone(),
            catalog.clone(),
        )]));
        compiled.narration_catalogs.insert(key.clone(), catalog);
        for program in compiled.stage_programs.programs.values_mut() {
            program.narration_ref = Some(key.clone());
            program.narration_digest = Some(digest.clone());
        }
    } else {
        let empty_digest = super::abi_project::compute_narration_digest(&BTreeMap::new());
        for program in compiled.stage_programs.programs.values_mut() {
            program.narration_ref = None;
            program.narration_digest = Some(empty_digest.clone());
        }
    }
    compiled.diagnostics.extend(diagnostics);
}

fn validate_target(
    target_ref: &str,
    cue_anchor: &str,
    targets: &NarrationTargetCatalog,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if is_private_target(target_ref) {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: NARRATION_TARGET_PRIVATE.to_string(),
            message: format!("narration target `{target_ref}` references a private runtime path"),
            source_path: Some(cue_anchor.to_string()),
        });
        return;
    }
    if !valid_target_shape(target_ref) || !targets.contains(target_ref) {
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: NARRATION_TARGET_UNKNOWN.to_string(),
            message: format!("narration target `{target_ref}` is not in a public target catalog"),
            source_path: Some(cue_anchor.to_string()),
        });
    }
}

fn valid_target_shape(target: &str) -> bool {
    if let Some(rest) = target.strip_prefix("admin:") {
        let parts: Vec<_> = rest.split('/').collect();
        return parts.len() == 3
            && valid_segment(parts[0])
            && valid_segment(parts[1])
            && parts[2]
                .strip_prefix("document_anchor:")
                .is_some_and(valid_segment);
    }
    let Some(rest) = target.strip_prefix("stage:") else {
        return false;
    };
    let parts: Vec<_> = rest.split('/').collect();
    match parts.as_slice() {
        [stage] => valid_segment(stage),
        [stage, leaf] => {
            valid_segment(stage)
                && [
                    "slide:",
                    "viewpoint:",
                    "t2_page:",
                    "document_anchor:",
                    "world_entity:",
                ]
                .iter()
                .any(|prefix| leaf.strip_prefix(prefix).is_some_and(valid_segment))
        }
        [stage, slide, slot] => {
            valid_segment(stage)
                && slide.strip_prefix("slide:").is_some_and(valid_segment)
                && slot.strip_prefix("slot:").is_some_and(valid_segment)
        }
        _ => false,
    }
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

fn is_private_target(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    target.starts_with('#')
        || lower.contains("document.")
        || lower.contains("queryselector")
        || lower.contains("mesh:")
        || lower.contains("panel_ref")
        || lower.contains("layout_region")
        || lower.contains("/t0/")
        || lower.contains("/t1/")
        || lower.contains("/region-")
}

fn collect_deck_targets(app_root: &Path, out: &mut NarrationTargetCatalog) {
    let root = app_root.join("src/presentation");
    if !root.is_dir() {
        return;
    }
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.ends_with(".deck.mdx"))
        })
    {
        let Ok(deck) = parse_deck_source_file(entry.path()) else {
            continue;
        };
        let stage_id = deck.frontmatter.id;
        let anchor = relative_path(app_root, entry.path());
        for slide in deck.slides {
            out.insert(
                format!("stage:{stage_id}/slide:{}", slide.id),
                format!("{anchor}:{}", slide.line),
            );
            for slot in slide.slots {
                out.insert(
                    format!("stage:{stage_id}/slide:{}/slot:{}", slide.id, slot.name),
                    format!("{anchor}:{}", slot.line),
                );
                out.insert(
                    format!("stage:{stage_id}/viewpoint:{}", slot.viewpoint_id),
                    format!("{anchor}:{}", slot.line),
                );
            }
        }
    }
}

fn collect_scene_targets(
    compiled: &CompiledApp,
    app_root: &Path,
    out: &mut NarrationTargetCatalog,
) {
    for stage in &compiled.stage_registry.stages {
        let stage_id = stage.id.as_str();
        let roots = [
            app_root.join(format!("src/scene/{stage_id}.mei")),
            app_root.join(format!("src/scene/{stage_id}")),
        ];
        for root in roots {
            if root.is_file() {
                collect_ids_from_file(app_root, &root, stage_id, out);
            } else if root.is_dir() {
                for entry in WalkDir::new(root)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|entry| {
                        entry.file_type().is_file()
                            && entry.path().extension().and_then(|ext| ext.to_str()) == Some("mei")
                    })
                {
                    collect_ids_from_file(app_root, entry.path(), stage_id, out);
                }
            }
        }
    }
}

fn collect_ids_from_file(
    app_root: &Path,
    path: &Path,
    stage_id: &str,
    out: &mut NarrationTargetCatalog,
) {
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };
    let anchor = relative_path(app_root, path);
    for id in quoted_assignments(source.as_str(), "id") {
        out.insert(format!("stage:{stage_id}/viewpoint:{id}"), anchor.clone());
        out.insert(
            format!("stage:{stage_id}/world_entity:{id}"),
            anchor.clone(),
        );
    }
    for scene in page_instance_scenes(source.as_str()) {
        out.insert(format!("stage:{stage_id}/t2_page:{scene}"), anchor.clone());
    }
    for document_anchor in inline_string_list(source.as_str(), "document_anchors") {
        out.insert(
            format!("stage:{stage_id}/document_anchor:{document_anchor}"),
            anchor.clone(),
        );
    }
}

fn collect_t2_targets(compiled: &CompiledApp, out: &mut NarrationTargetCatalog) {
    for stage in &compiled.stage_registry.stages {
        for page in compiled.build_t2_page_index.pages.values() {
            out.insert(
                format!("stage:{}/t2_page:{}", stage.id, page.scene_id),
                page.page_file.clone(),
            );
        }
    }
}

fn collect_admin_targets(app_root: &Path, app_id: &str, out: &mut NarrationTargetCatalog) {
    let crate::mei_config::AdminDiscoverOutcome::Ok(projection) =
        crate::mei_config::discover_app_admin_resources(app_root, app_id)
    else {
        return;
    };
    for resource in projection.resources {
        let entry = resource.registry_entry;
        let scene_anchor = resource.page_program.root.scene_ref().replace('.', "/");
        let scene_path = app_root.join(format!("src/scene/{scene_anchor}.mei"));
        let Ok(source) = fs::read_to_string(&scene_path) else {
            continue;
        };
        for anchor_id in inline_string_list(source.as_str(), "document_anchors") {
            out.insert(
                format!(
                    "admin:{}/{}/document_anchor:{anchor_id}",
                    entry.resource_id, entry.module_id
                ),
                relative_path(app_root, &scene_path),
            );
        }
    }
}

fn page_instance_scenes(source: &str) -> Vec<String> {
    let mut scenes = Vec::new();
    let mut in_page = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("page_instance(") {
            in_page = true;
        }
        if in_page {
            if let Some(value) = quoted_assignment(trimmed, "scene") {
                scenes.push(value);
            }
            if trimmed == ")" {
                in_page = false;
            }
        }
    }
    scenes
}

fn quoted_assignments(source: &str, key: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| quoted_assignment(line.trim(), key))
        .collect()
}

fn quoted_assignment(line: &str, key: &str) -> Option<String> {
    let after = line
        .strip_prefix(key)?
        .trim_start()
        .strip_prefix('=')?
        .trim();
    let value = after.strip_prefix('"')?;
    Some(value.split('"').next()?.to_string())
}

fn inline_string_list(source: &str, key: &str) -> Vec<String> {
    let Some(start) = source.find(key) else {
        return Vec::new();
    };
    let rest = &source[start + key.len()..];
    let Some(open) = rest.find('[') else {
        return Vec::new();
    };
    let rest = &rest[open + 1..];
    let Some(close) = rest.find(']') else {
        return Vec::new();
    };
    rest[..close]
        .split(',')
        .map(str::trim)
        .filter_map(|value| value.strip_prefix('"').and_then(|v| v.strip_suffix('"')))
        .filter(|value| valid_segment(value))
        .map(str::to_string)
        .collect()
}

fn relative_path(app_root: &Path, path: &Path) -> String {
    path.strip_prefix(app_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn digest_serializable<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn validates_frozen_stage_and_admin_target_shapes() {
        assert!(valid_target_shape("stage:home/viewpoint:warnings_main"));
        assert!(valid_target_shape("stage:slides/slide:mission/slot:items"));
        assert!(valid_target_shape(
            "admin:demo/overview/document_anchor:basic"
        ));
        assert!(!valid_target_shape("admin:demo/overview/anchor:basic"));
        assert!(!valid_target_shape("stage:home/slot:warnings_main"));
    }

    #[test]
    fn discovers_only_narration_track_suffix_recursively() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        fs::create_dir_all(root.join("src/narration/nested")).expect("mkdir");
        fs::write(root.join("src/narration/a.track.mdx"), "").expect("write");
        fs::write(root.join("src/narration/nested/b.track.mdx"), "").expect("write");
        fs::write(root.join("src/narration/legacy.scene.mdx"), "").expect("write");
        let paths = discover_narration_track_paths(root);
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().all(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".track.mdx"))
        }));
    }

    #[test]
    fn target_diagnostics_distinguish_private_and_unknown() {
        let targets = NarrationTargetCatalog::default();
        let mut diagnostics = Vec::new();
        validate_target(
            "dom.querySelector",
            "src/narration/a.track.mdx:1",
            &targets,
            &mut diagnostics,
        );
        validate_target(
            "stage:home/viewpoint:missing",
            "src/narration/a.track.mdx:2",
            &targets,
            &mut diagnostics,
        );
        assert_eq!(diagnostics[0].code, NARRATION_TARGET_PRIVATE);
        assert_eq!(diagnostics[1].code, NARRATION_TARGET_UNKNOWN);
    }
}
