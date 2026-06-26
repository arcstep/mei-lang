
use serde::{Deserialize, Serialize};



pub(super) const MIN_UPLOAD_CHUNK_BYTES: usize = 1024 * 1024;
pub(super) const MAX_UPLOAD_CHUNK_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct UploadDeleteQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadDownloadQuery {
    pub path: String,
    #[serde(default)]
    pub inline: bool,
    #[serde(default)]
    pub match_basename: bool,
    pub basename: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkStatusQuery {
    pub upload_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkPutQuery {
    pub upload_id: String,
    pub index: usize,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkInitRequest {
    pub file_name: String,
    pub dir: Option<String>,
    pub size_bytes: u64,
    pub chunk_size: usize,
    pub last_modified_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct UploadChunkCompleteRequest {
    pub upload_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadMoveRequest {
    pub from_path: String,
    pub to_dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UploadDirCreateRequest {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct UploadRenameRequest {
    pub from_path: String,
    pub to_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct UploadChunkSessionMeta {
    pub(super) upload_id: String,
    pub(super) rel_path: String,
    pub(super) file_name: String,
    pub(super) size_bytes: u64,
    pub(super) chunk_size: usize,
    pub(super) total_chunks: usize,
    pub(super) last_modified_ms: Option<u64>,
}

