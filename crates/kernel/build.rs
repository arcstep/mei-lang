use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

fn stable_hash(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn hash_source_tree(root: &Path) -> String {
    let mut parts = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if !rel.ends_with(".rs") {
            continue;
        }
        let content = fs::read(path).unwrap_or_default();
        parts.push(format!(
            "{rel}:{}",
            stable_hash(&String::from_utf8_lossy(&content))
        ));
        println!("cargo:rerun-if-changed={}", path.display());
    }
    parts.sort();
    stable_hash(&parts.join("\n"))
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let revision = hash_source_tree(&manifest_dir.join("src"));
    println!("cargo:rustc-env=MEI_KERNEL_SOURCE_REVISION={revision}");
}
