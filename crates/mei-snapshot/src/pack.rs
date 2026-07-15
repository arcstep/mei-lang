use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::manifest::{
    DataModeHint, ManifestFileEntry, SnapshotManifest, FORMAT_NAME, FORMAT_VERSION,
};
use crate::paths::{resolve_app_env_root, resolve_bundle_path};

#[derive(Debug, Clone)]
pub struct PackOptions {
    pub workspace: PathBuf,
    pub app_id: String,
    pub out: PathBuf,
    pub include_data: bool,
    pub include_cache: bool,
    pub default_scene: Option<String>,
    pub compiler_version: Option<String>,
}

pub fn pack_snapshot(opts: &PackOptions) -> anyhow::Result<SnapshotManifest> {
    let env_root = resolve_app_env_root(&opts.workspace, &opts.app_id)?;
    let bundle = resolve_bundle_path(&env_root, &opts.app_id)?;

    let staging = tempfile_dir(&opts.out)?;
    let exchange_dir = staging.join("exchange");
    fs::create_dir_all(&exchange_dir)?;
    let bundle_name = format!("{}.meibundle", opts.app_id);
    let staged_bundle = exchange_dir.join(&bundle_name);
    fs::copy(&bundle, &staged_bundle)?;

    let mut data_mode = DataModeHint::Static;
    if opts.include_data {
        let src = env_root.join("var").join("data-snapshots");
        if src.is_dir() {
            copy_dir_contents(&src, &staging.join("data-snapshots"))?;
            data_mode = DataModeHint::Eval;
        }
    }
    if opts.include_cache {
        let mrg = env_root.join("var").join("mrg");
        if mrg.is_dir() {
            copy_dir_contents(&mrg, &staging.join("cache").join("mrg"))?;
        }
    }

    let mut files = collect_file_entries(&staging)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));

    let manifest = SnapshotManifest {
        format: FORMAT_NAME.to_string(),
        format_version: FORMAT_VERSION,
        app_id: opts.app_id.clone(),
        default_scene: opts.default_scene.clone(),
        compiler_version: opts.compiler_version.clone(),
        data_mode_hint: data_mode,
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        files,
    };
    manifest.validate()?;

    let manifest_path = staging.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    if let Some(parent) = opts.out.parent() {
        fs::create_dir_all(parent)?;
    }
    write_zip(&staging, &opts.out)?;
    let _ = fs::remove_dir_all(&staging);
    Ok(manifest)
}

fn tempfile_dir(out: &Path) -> anyhow::Result<PathBuf> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = format!(
        ".mei-snapshot-staging-{}",
        std::process::id()
    );
    let dir = parent.join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn copy_dir_contents(src: &Path, dst: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path == src {
            continue;
        }
        let rel = path.strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, &target)?;
        }
    }
    Ok(())
}

fn collect_file_entries(root: &Path) -> anyhow::Result<Vec<ManifestFileEntry>> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "manifest.json" {
            continue;
        }
        let bytes = fs::metadata(path)?.len();
        let sha256 = sha256_file(path)?;
        out.push(ManifestFileEntry {
            path: rel,
            sha256: Some(sha256),
            bytes: Some(bytes),
        });
    }
    Ok(out)
}

fn sha256_file(path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn write_zip(staging: &Path, out: &Path) -> anyhow::Result<()> {
    let file = File::create(out)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Ensure manifest is first for easier inspection.
    let manifest = staging.join("manifest.json");
    if manifest.is_file() {
        zip.start_file("manifest.json", options)?;
        let mut f = File::open(&manifest)?;
        std::io::copy(&mut f, &mut zip)?;
    }

    for entry in WalkDir::new(staging).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = path
            .strip_prefix(staging)?
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "manifest.json" {
            continue;
        }
        zip.start_file(&rel, options)?;
        let mut f = File::open(path)?;
        std::io::copy(&mut f, &mut zip)?;
    }
    zip.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unpack::unpack_snapshot;

    #[test]
    fn pack_and_unpack_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("mei-snap-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let ws = tmp.join("ws");
        let app = "demo";
        let env = ws.join("apps").join(app).join("env").join("WS-1");
        let exchange = env.join("build").join("exchange");
        fs::create_dir_all(&exchange).unwrap();
        fs::write(exchange.join("demo.meibundle"), b"fake-bundle").unwrap();
        fs::create_dir_all(env.join("var").join("data-snapshots")).unwrap();
        fs::write(
            env.join("var").join("data-snapshots").join("a.parquet"),
            b"pq",
        )
        .unwrap();

        let out = tmp.join("demo.mei-snapshot.zip");
        let manifest = pack_snapshot(&PackOptions {
            workspace: ws,
            app_id: app.into(),
            out: out.clone(),
            include_data: true,
            include_cache: false,
            default_scene: Some("home".into()),
            compiler_version: Some("test".into()),
        })
        .unwrap();
        assert_eq!(manifest.app_id, "demo");
        assert_eq!(manifest.data_mode_hint, DataModeHint::Eval);

        let dest = tmp.join("unpacked");
        let result = unpack_snapshot(&out, &dest).unwrap();
        assert_eq!(result.manifest.app_id, "demo");
        assert!(result.bundle_path.is_file());
        assert!(dest.join("data-snapshots").join("a.parquet").is_file());
        let _ = fs::remove_dir_all(&tmp);
    }
}
