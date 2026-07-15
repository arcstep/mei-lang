use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::manifest::SnapshotManifest;

#[derive(Debug, Clone)]
pub struct UnpackResult {
    pub dest: PathBuf,
    pub manifest: SnapshotManifest,
    pub bundle_path: PathBuf,
}

pub fn unpack_snapshot(archive: &Path, into: &Path) -> anyhow::Result<UnpackResult> {
    if into.exists() {
        // Allow empty dir; refuse non-empty to avoid clobber.
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

    let bundle_rel = manifest
        .files
        .iter()
        .find(|f| f.path.starts_with("exchange/") && f.path.ends_with(".meibundle"))
        .map(|f| f.path.clone())
        .ok_or_else(|| anyhow::anyhow!("no exchange/*.meibundle in manifest"))?;
    let bundle_path = into.join(&bundle_rel);
    if !bundle_path.is_file() {
        anyhow::bail!("bundle missing after unpack: {}", bundle_path.display());
    }

    Ok(UnpackResult {
        dest: into.to_path_buf(),
        manifest,
        bundle_path,
    })
}
