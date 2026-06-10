use std::env;
use std::fs;
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

fn utc_timestamp() -> String {
    let repo_root = repo_root_from_manifest();
    run_git(&repo_root, &["log", "-1", "--format=%cI"])
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn repo_root_from_manifest() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .parent()
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
    let target_tag = env::var("MEI_BUILD_TARGET_TAG")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let cargo_package_version =
        env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    let major_version = cargo_package_version
        .split('.')
        .next()
        .unwrap_or("0")
        .to_string();

    let git_commit_short = env_override("MEI_GIT_COMMIT_SHORT")
        .or_else(|| run_git(&repo_root, &["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let git_commit_full = env_override("MEI_GIT_COMMIT_FULL")
        .or_else(|| run_git(&repo_root, &["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let git_branch = env_override("MEI_GIT_BRANCH")
        .or_else(|| run_git(&repo_root, &["rev-parse", "--abbrev-ref", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let git_dirty = bool_env_override("MEI_GIT_DIRTY").unwrap_or_else(|| git_dirty(&repo_root));
    let internal_version = if git_dirty {
        format!("{git_commit_short}-dirty")
    } else {
        git_commit_short.clone()
    };
    let build_version = format!("{cargo_package_version}+{internal_version}");
    let build_timestamp_utc =
        env_override("MEI_BUILD_TIMESTAMP_UTC").unwrap_or_else(utc_timestamp);

    emit_rustc_env("MEI_MAJOR_VERSION", &major_version);
    emit_rustc_env("MEI_INTERNAL_VERSION", &internal_version);
    emit_rustc_env("MEI_BUILD_VERSION", &build_version);
    emit_rustc_env("MEI_GIT_COMMIT_SHORT", &git_commit_short);
    emit_rustc_env("MEI_GIT_COMMIT_FULL", &git_commit_full);
    emit_rustc_env("MEI_GIT_BRANCH", &git_branch);
    emit_rustc_env("MEI_GIT_DIRTY", if git_dirty { "true" } else { "false" });
    emit_rustc_env("MEI_BUILD_TARGET_TAG", &target_tag);
    emit_rustc_env("MEI_BUILD_TIMESTAMP_UTC", &build_timestamp_utc);
    emit_rustc_env("MEI_CARGO_PACKAGE_VERSION", &cargo_package_version);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let build_info_json = format!(
        r#"{{
  "component": "mei-lang-server",
  "build_version": "{build_version}",
  "cargo_package_version": "{cargo_package_version}",
  "major_version": "{major_version}",
  "internal_version": "{internal_version}",
  "git_commit_short": "{git_commit_short}",
  "git_commit_full": "{git_commit_full}",
  "git_branch": "{git_branch}",
  "git_dirty": {git_dirty},
  "build_target_tag": "{target_tag}",
  "build_timestamp_utc": "{build_timestamp_utc}",
  "deploy_package_root": "/home/spbjw/deploy/mei-lang"
}}"#
    );
    fs::write(out_dir.join("build-info.json"), build_info_json).expect("write build-info.json");

    println!("cargo:rerun-if-changed=../Cargo.toml");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
    println!("cargo:rerun-if-env-changed=MEI_BUILD_TARGET_TAG");
    println!("cargo:rerun-if-env-changed=MEI_GIT_COMMIT_SHORT");
    println!("cargo:rerun-if-env-changed=MEI_GIT_COMMIT_FULL");
    println!("cargo:rerun-if-env-changed=MEI_GIT_BRANCH");
    println!("cargo:rerun-if-env-changed=MEI_GIT_DIRTY");
    println!("cargo:rerun-if-env-changed=MEI_BUILD_TIMESTAMP_UTC");
}
