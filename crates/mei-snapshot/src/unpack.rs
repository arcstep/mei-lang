use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::manifest::SnapshotManifest;
use crate::resources::ResourcesDocument;

#[derive(Debug, Clone)]
pub struct UnpackResult {
    pub dest: PathBuf,
    pub manifest: SnapshotManifest,
    /// Primary / first app bundle path (v1 or v2).
    pub bundle_path: PathBuf,
    /// All app bundles for v2 (also length 1 for v1).
    pub app_bundle_paths: Vec<(String, PathBuf)>,
    pub resources: Option<ResourcesDocument>,
}

pub fn unpack_snapshot(archive: &Path, into: &Path) -> anyhow::Result<UnpackResult> {
    if into.exists() {
        if into.is_dir() {
            let mut entries = fs::read_dir(into)?;
            if entries.next().is_some() {
                anyhow::bail!(
                    "unpack destination is not empty: {}; choose an empty dir",
                    into.display()
                );
            }
        } else {
            anyhow::bail!("unpack destination exists and is not a directory");
        }
    } else {
        fs::create_dir_all(into)?;
    }

    let file = File::open(archive)
        .map_err(|e| anyhow::anyhow!("open archive {}: {e}", archive.display()))?;
    let mut zip = ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry
            .enclosed_name()
            .ok_or_else(|| anyhow::anyhow!("invalid zip entry name"))?
            .to_path_buf();
        let out_path = into.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut outfile = File::create(&out_path)?;
        std::io::copy(&mut entry, &mut outfile)?;
    }

    let manifest_path = into.join("manifest.json");
    let mut raw = String::new();
    File::open(&manifest_path)?.read_to_string(&mut raw)?;
    let manifest: SnapshotManifest = serde_json::from_str(&raw)?;
    manifest.validate()?;

    let app_entries = manifest.app_entries();
    let mut app_bundle_paths = Vec::new();
    for app in &app_entries {
        let bundle_path = into.join(&app.bundle_path);
        if !bundle_path.is_file() {
            anyhow::bail!("bundle missing after unpack: {}", bundle_path.display());
        }
        app_bundle_paths.push((app.app_id.clone(), bundle_path));
    }
    let bundle_path = app_bundle_paths
        .first()
        .map(|(_, p)| p.clone())
        .ok_or_else(|| anyhow::anyhow!("no app bundles in snapshot"))?;

    let resources = {
        let path = into.join("resources.json");
        if path.is_file() {
            let text = fs::read_to_string(&path)?;
            Some(serde_json::from_str(&text)?)
        } else {
            None
        }
    };

    Ok(UnpackResult {
        dest: into.to_path_buf(),
        manifest,
        bundle_path,
        app_bundle_paths,
        resources,
    })
}
