use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub fn compute_workspace_digest(workspace: &Path, app_id: &str, templates_rel: &str) -> String {
    let mut hasher = Sha256::new();
    let src_root = workspace.join("apps").join(app_id).join("src");
    hash_mei_tree(&mut hasher, &src_root);
    let templates = workspace.join(templates_rel);
    if templates.is_dir() {
        hash_mei_tree(&mut hasher, &templates);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_mei_tree(hasher: &mut Sha256, root: &Path) {
    if !root.is_dir() {
        return;
    }
    let mut paths = WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "mei"))
        .map(|e| e.path().to_path_buf())
        .collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        if let Ok(mut file) = std::fs::File::open(&path) {
            let mut buf = Vec::new();
            if file.read_to_end(&mut buf).is_ok() {
                hasher.update(&buf);
            }
        }
        hasher.update([0u8]);
    }
}
