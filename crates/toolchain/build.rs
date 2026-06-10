use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_git(repo_root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn git_dirty(repo_root: &Path) -> bool {
    run_git(repo_root, &["status", "--porcelain"])
        .map(|status| !status.is_empty())
        .unwrap_or(false)
}

fn repo_root_from_manifest() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("mei-lang repo root")
        .to_path_buf()
}

fn emit_rustc_env(key: &str, value: &str) {
    println!("cargo:rustc-env={key}={value}");
}

fn env_override(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bool_env_override(key: &str) -> Option<bool> {
    env_override(key).and_then(|value| match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    })
}

fn main() {
    let repo_root = repo_root_from_manifest();
    let cargo_package_version =
        env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let major_version = cargo_package_version
        .split('.')
        .next()
        .unwrap_or("0")
        .to_string();
    let compatibility_line = format!("mei-{major_version}");
    let target_triple = env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    let git_commit_short = env_override("MEI_GIT_COMMIT_SHORT")
        .or_else(|| run_git(&repo_root, &["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let git_commit_full = env_override("MEI_GIT_COMMIT_FULL")
        .or_else(|| run_git(&repo_root, &["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let git_dirty = bool_env_override("MEI_GIT_DIRTY").unwrap_or_else(|| git_dirty(&repo_root));
    let internal_version = if git_dirty {
        format!("{git_commit_short}-dirty")
    } else {
        git_commit_short.clone()
    };
    let build_version = format!("{cargo_package_version}+{internal_version}");
    let build_timestamp_utc = env_override("MEI_BUILD_TIMESTAMP_UTC")
        .or_else(|| run_git(&repo_root, &["log", "-1", "--format=%cI"]).filter(|value| !value.is_empty()))
        .unwrap_or_else(|| "unknown".to_string());

    emit_rustc_env("MEI_CARGO_PACKAGE_VERSION", &cargo_package_version);
    emit_rustc_env("MEI_GIT_COMMIT_SHORT", &git_commit_short);
    emit_rustc_env("MEI_GIT_COMMIT_FULL", &git_commit_full);
    emit_rustc_env("MEI_GIT_DIRTY", if git_dirty { "true" } else { "false" });
    emit_rustc_env("MEI_BUILD_VERSION", &build_version);
    emit_rustc_env("MEI_BUILD_TIMESTAMP_UTC", &build_timestamp_utc);
    emit_rustc_env("MEI_TARGET_TRIPLE", &target_triple);
    emit_rustc_env("MEI_COMPATIBILITY_LINE", &compatibility_line);

    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join("Cargo.toml").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join(".git/HEAD").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        repo_root.join(".git/index").display()
    );
    println!("cargo:rerun-if-env-changed=MEI_GIT_COMMIT_SHORT");
    println!("cargo:rerun-if-env-changed=MEI_GIT_COMMIT_FULL");
    println!("cargo:rerun-if-env-changed=MEI_GIT_DIRTY");
    println!("cargo:rerun-if-env-changed=MEI_BUILD_TIMESTAMP_UTC");
}
