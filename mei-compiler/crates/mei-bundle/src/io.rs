use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use mei_graph::GraphBlock;
use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    build_manifest, exchange_from_outcome, MeiBundleManifest, MeiCompileExchange, BLOCKS_ZST_PATH,
    MANIFEST_PATH,
};

#[derive(Debug, Error)]
pub enum WriteBundleError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("zstd error: {0}")]
    Zstd(String),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

#[derive(Debug, Error)]
pub enum ReadBundleError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("zstd error: {0}")]
    Zstd(String),
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone)]
pub struct BundleStats {
    pub manifest: MeiBundleManifest,
    pub bundle_bytes: u64,
    pub blocks_json_bytes: u64,
    pub blocks_zstd_bytes: u64,
}

pub fn blocks_json_compact(blocks: &[GraphBlock]) -> Result<Vec<u8>, serde_json::Error> {
    let text = serde_json::to_string(blocks)?;
    Ok(text.into_bytes())
}

pub fn zstd_compress(input: &[u8]) -> Result<Vec<u8>, WriteBundleError> {
    zstd::encode_all(input, 3).map_err(|e| WriteBundleError::Zstd(e.to_string()))
}

pub fn zstd_decompress(input: &[u8]) -> Result<Vec<u8>, ReadBundleError> {
    zstd::decode_all(input).map_err(|e| ReadBundleError::Zstd(e.to_string()))
}

pub fn write_bundle(
    exchange: &MeiCompileExchange,
    workspace_digest: &str,
    compiler_version: &str,
    path: &Path,
    emit_debug_sidecar: bool,
) -> Result<BundleStats, WriteBundleError> {
    let compiled_at_ms = current_time_ms();
    let manifest = build_manifest(
        exchange,
        workspace_digest,
        compiler_version,
        compiled_at_ms,
    );
    let blocks_json = blocks_json_compact(&exchange.blocks)?;
    let blocks_zstd = zstd_compress(&blocks_json)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(path)?;
    let mut zip = ZipWriter::new(file);
    let store = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    let manifest_json = serde_json::to_vec_pretty(&manifest)?;
    zip.start_file(MANIFEST_PATH, store)?;
    zip.write_all(&manifest_json)?;

    zip.start_file(BLOCKS_ZST_PATH, store)?;
    zip.write_all(&blocks_zstd)?;

    zip.finish()?;

    let bundle_bytes = std::fs::metadata(path)?.len();
    if emit_debug_sidecar {
        write_debug_sidecar(path, &exchange.blocks)?;
    }
    Ok(BundleStats {
        manifest,
        bundle_bytes,
        blocks_json_bytes: blocks_json.len() as u64,
        blocks_zstd_bytes: blocks_zstd.len() as u64,
    })
}

pub fn write_bundle_from_outcome(
    outcome: &mei_graph::CompileOutcome,
    workspace_digest: &str,
    compiler_version: &str,
    path: &Path,
    emit_debug_sidecar: bool,
) -> Result<BundleStats, WriteBundleError> {
    let exchange = exchange_from_outcome(outcome);
    write_bundle(
        &exchange,
        workspace_digest,
        compiler_version,
        path,
        emit_debug_sidecar,
    )
}

/// Write human-readable compile blocks next to `.meibundle` for source→artifact debugging.
pub fn write_debug_sidecar(bundle_path: &Path, blocks: &[GraphBlock]) -> Result<(), WriteBundleError> {
    let file_name = bundle_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("bundle");
    let sidecar = bundle_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{file_name}.blocks.pretty.json"));
    let pretty = serde_json::to_vec_pretty(blocks)?;
    std::fs::write(&sidecar, pretty)?;
    Ok(())
}

pub fn read_bundle(path: &Path) -> Result<(MeiBundleManifest, Vec<GraphBlock>), ReadBundleError> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut manifest_raw = Vec::new();
    {
        let mut manifest_file = archive.by_name(MANIFEST_PATH).map_err(|_| {
            ReadBundleError::Invalid(format!("missing {MANIFEST_PATH} in bundle"))
        })?;
        manifest_file.read_to_end(&mut manifest_raw)?;
    }
    let manifest: MeiBundleManifest = serde_json::from_slice(&manifest_raw)?;

    if manifest.bundle_schema_version != crate::BUNDLE_SCHEMA_VERSION {
        return Err(ReadBundleError::Invalid(format!(
            "unsupported bundle_schema_version: {}",
            manifest.bundle_schema_version
        )));
    }

    let blocks_path = if manifest.blocks_path.is_empty() {
        BLOCKS_ZST_PATH.to_string()
    } else {
        manifest.blocks_path.clone()
    };

    let mut zst_raw = Vec::new();
    {
        let mut blocks_file = archive.by_name(&blocks_path).map_err(|_| {
            ReadBundleError::Invalid(format!("missing {blocks_path} in bundle"))
        })?;
        blocks_file.read_to_end(&mut zst_raw)?;
    }

    let blocks_json = zstd_decompress(&zst_raw)?;
    let blocks: Vec<GraphBlock> = serde_json::from_slice(&blocks_json)?;

    if blocks.len() != manifest.block_count {
        return Err(ReadBundleError::Invalid(format!(
            "block_count mismatch: manifest {} vs payload {}",
            manifest.block_count,
            blocks.len()
        )));
    }

    Ok((manifest, blocks))
}

pub fn bundle_stats(path: &Path) -> Result<BundleStats, ReadBundleError> {
    let (manifest, blocks) = read_bundle(path)?;
    let blocks_json = blocks_json_compact(&blocks).map_err(ReadBundleError::Json)?;
    let bundle_bytes = std::fs::metadata(path)?.len();
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let blocks_path = if manifest.blocks_path.is_empty() {
        BLOCKS_ZST_PATH.to_string()
    } else {
        manifest.blocks_path.clone()
    };
    let mut zst_raw = Vec::new();
    archive
        .by_name(&blocks_path)
        .map_err(|_| ReadBundleError::Invalid(format!("missing {blocks_path} in bundle")))?
        .read_to_end(&mut zst_raw)?;

    Ok(BundleStats {
        manifest,
        bundle_bytes,
        blocks_json_bytes: blocks_json.len() as u64,
        blocks_zstd_bytes: zst_raw.len() as u64,
    })
}

fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
