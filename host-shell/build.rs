use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use sha2::{Digest, Sha256};

struct PagePackSpec {
    pack_id: &'static str,
    page_id: &'static str,
    title: &'static str,
    source_anchor: &'static str,
    scene_ref: &'static str,
    pack_source: &'static str,
    source_contract: &'static [&'static str],
    aot_template: &'static str,
    required_aot_slots: &'static [&'static str],
    generated_file: &'static str,
    const_prefix: &'static str,
}

const HOME_PACK: PagePackSpec = PagePackSpec {
    pack_id: "host.home",
    page_id: "home",
    title: "MeiLang 工作区",
    source_anchor: "host://pagepacks/home.page.mdx",
    scene_ref: "host/home",
    pack_source: "pagepacks/home.page.mdx",
    source_contract: &[
        "page_id: home",
        "profile: page",
        "@template(use=\"host-home\")",
        "@slot(id=\"workspace_line\")",
        "@slot(id=\"app_cards\")",
    ],
    aot_template: r#"
<section class="mei-host-shell__home" data-mei-pagepack="{{mei:pack_id}}" data-mei-pagepack-digest="{{mei:digest}}" data-mei-page-surface="{{mei:surface}}">
  {{mei:workspace_line}}
  {{mei:app_cards}}
</section>
"#,
    required_aot_slots: &[
        "{{mei:pack_id}}",
        "{{mei:digest}}",
        "{{mei:surface}}",
        "{{mei:workspace_line}}",
        "{{mei:app_cards}}",
    ],
    generated_file: "host_home_page_pack.rs",
    const_prefix: "HOME",
};

const RUNTIME_PACK: PagePackSpec = PagePackSpec {
    pack_id: "host.runtime",
    page_id: "runtime",
    title: "运行中心",
    source_anchor: "host://pagepacks/runtime.page.mdx",
    scene_ref: "host/runtime",
    pack_source: "pagepacks/runtime.page.mdx",
    source_contract: &[
        "page_id: runtime",
        "profile: page",
        "@template(use=\"host-runtime\")",
        "@slot(id=\"host_tools\")",
        "@slot(id=\"runtime_control\")",
    ],
    aot_template: r#"
<section class="mei-host-shell__runtime" data-mei-pagepack="{{mei:pack_id}}" data-mei-pagepack-digest="{{mei:digest}}" data-mei-page-surface="{{mei:surface}}">
  {{mei:host_tools}}
  {{mei:runtime_control}}
</section>
"#,
    required_aot_slots: &[
        "{{mei:pack_id}}",
        "{{mei:digest}}",
        "{{mei:surface}}",
        "{{mei:host_tools}}",
        "{{mei:runtime_control}}",
    ],
    generated_file: "host_runtime_page_pack.rs",
    const_prefix: "RUNTIME",
};

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

fn emit_git_rerun_paths(repo_root: &std::path::Path) {
    let git_dir = repo_root.join(".git");
    let head = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    println!("cargo:rerun-if-changed={}", git_dir.join("packed-refs").display());
    if let Ok(value) = fs::read_to_string(&head) {
        if let Some(reference) = value.trim().strip_prefix("ref: ") {
            println!(
                "cargo:rerun-if-changed={}",
                git_dir.join(reference).display()
            );
        }
    }
}

fn canonical_page_pack_payload(spec: &PagePackSpec, template: &str) -> String {
    format!(
        "host-page-pack-v1\npack_id:{}\npage_id:{}\ntitle:{}\nsource_anchor:{}\nsurface:document\nscene_ref:{}\nadmin_resource_id:\naot_body:\n{}",
        spec.pack_id, spec.page_id, spec.title, spec.source_anchor, spec.scene_ref, template
    )
}

fn compile_page_pack(manifest_dir: &std::path::Path, spec: &PagePackSpec) {
    let source_path = manifest_dir.join(spec.pack_source);
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|err| panic!("read {}: {err}", source_path.display()));
    for contract in spec.source_contract {
        assert!(
            source.contains(contract),
            "Host {} Page source must declare {contract}",
            spec.page_id
        );
    }
    assert!(
        !source.contains('<') && !source.contains('>'),
        "Host {} Page source must use Page directives, not raw HTML",
        spec.page_id
    );
    let template = spec.aot_template.trim().to_string();

    let mut template_without_slots = template.clone();
    for slot in spec.required_aot_slots {
        let count = template.matches(slot).count();
        assert_eq!(
            count, 1,
            "Host {} PagePack must contain {slot} exactly once",
            spec.page_id
        );
        template_without_slots = template_without_slots.replace(slot, "");
    }
    assert!(
        !template_without_slots.contains("{{mei:"),
        "Host {} PagePack contains an unknown slot",
        spec.page_id
    );
    assert!(
        !template.contains("<script"),
        "Host {} PagePack body must not contain scripts",
        spec.page_id
    );

    let digest = format!(
        "sha256:{:x}",
        Sha256::digest(canonical_page_pack_payload(spec, &template).as_bytes())
    );
    let prefix = spec.const_prefix;
    let generated = format!(
        "pub const {prefix}_PAGE_PACK_ID: &str = {pack_id:?};\n\
         pub const {prefix}_PAGE_PACK_DIGEST: &str = {digest:?};\n\
         pub const {prefix}_PAGE_PACK_TEMPLATE: &str = {template:?};\n\
         pub const {prefix}_PAGE_ID: &str = {page_id:?};\n\
         pub const {prefix}_PAGE_TITLE: &str = {title:?};\n\
         pub const {prefix}_PAGE_SOURCE_ANCHOR: &str = {source_anchor:?};\n\
         pub const {prefix}_PAGE_SCENE_REF: &str = {scene_ref:?};\n",
        pack_id = spec.pack_id,
        digest = digest,
        template = template,
        page_id = spec.page_id,
        title = spec.title,
        source_anchor = spec.source_anchor,
        scene_ref = spec.scene_ref,
    );
    let output_path =
        PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join(spec.generated_file);
    if fs::read_to_string(&output_path).ok().as_deref() != Some(generated.as_str()) {
        fs::write(&output_path, generated)
            .unwrap_or_else(|err| panic!("write {}: {err}", output_path.display()));
    }

    println!("cargo:rerun-if-changed={}", spec.pack_source);
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .expect("mei-lang repo root")
        .to_path_buf();

    let cargo_package_version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
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
    emit_git_rerun_paths(repo_root.as_path());
    println!("cargo:rerun-if-env-changed=MEI_GIT_COMMIT_SHORT");
    println!("cargo:rerun-if-env-changed=MEI_GIT_BRANCH");
    println!("cargo:rerun-if-env-changed=MEI_GIT_DIRTY");
    compile_page_pack(manifest_dir.as_path(), &HOME_PACK);
    compile_page_pack(manifest_dir.as_path(), &RUNTIME_PACK);
}
