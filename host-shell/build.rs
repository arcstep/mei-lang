use std::env;
use std::path::PathBuf;
use std::process::Command;

fn run_git(repo_root: &std::path::Path, args: &[&str]) -> Option<String> {
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

fn git_dirty(repo_root: &std::path::Path) -> bool {
    run_git(repo_root, &["status", "--porcelain"])
        .map(|status| !status.is_empty())
        .unwrap_or(false)
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .expect("mei-lang repo root")
        .to_path_buf();

    let cargo_package_version =
        env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "2.0.3".to_string());
    let git_commit_short = env::var("MEI_GIT_COMMIT_SHORT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| run_git(&repo_root, &["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let git_dirty = env::var("MEI_GIT_DIRTY")
        .ok()
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or_else(|| git_dirty(&repo_root));
    let internal_version = if git_dirty {
        format!("{git_commit_short}-dirty")
    } else {
        git_commit_short.clone()
    };
    let git_branch = env::var("MEI_GIT_BRANCH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| run_git(&repo_root, &["rev-parse", "--abbrev-ref", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_string());
    let build_version = format!("{cargo_package_version}+{internal_version}");

    println!("cargo:rustc-env=MEI_BUILD_VERSION={build_version}");
    println!("cargo:rustc-env=MEI_CARGO_PACKAGE_VERSION={cargo_package_version}");
    println!("cargo:rustc-env=MEI_GIT_COMMIT_SHORT={git_commit_short}");
    println!("cargo:rustc-env=MEI_GIT_BRANCH={git_branch}");
    println!(
        "cargo:rustc-env=MEI_GIT_DIRTY={}",
        if git_dirty { "true" } else { "false" }
    );
    println!("cargo:rerun-if-changed=../Cargo.toml");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/index");
}
