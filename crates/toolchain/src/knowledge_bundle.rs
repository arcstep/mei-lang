use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::json;

pub const KNOWLEDGE_BUNDLE_SCHEMA_VERSION: &str = "mei-knowledge-bundle-v1";

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeAssetDescriptor {
    pub id: String,
    pub surface: String,
    pub topic: String,
    pub kind: String,
    pub title: String,
    pub relative_path: String,
    pub install_relative_path: String,
    pub summary: String,
    pub injection_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeBundleDescriptor {
    pub schema_version: String,
    pub bundle_id: String,
    pub surface: String,
    pub package_root: String,
    pub install_dir_rel: String,
    pub primary_entry_ids: Vec<String>,
    pub available_topics: Vec<String>,
    pub assets: Vec<KnowledgeAssetDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeAssetContent {
    pub descriptor: KnowledgeAssetDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Clone, Copy)]
struct AssetSeed {
    id: &'static str,
    topic: &'static str,
    kind: &'static str,
    title: &'static str,
    rel_path: &'static str,
    install_rel_path: &'static str,
    summary: &'static str,
    injection_roles: &'static [&'static str],
}

fn normalize_surface(surface: &str) -> Option<&'static str> {
    match surface.trim().to_ascii_lowercase().as_str() {
        "author" => Some("author"),
        "access" => Some("access"),
        _ => None,
    }
}

fn author_assets() -> Vec<AssetSeed> {
    vec![
        AssetSeed {
            id: "meilang_author_skill",
            topic: "workflow",
            kind: "skill_entry",
            title: "MeiLang Author Skill",
            rel_path: "guides/author-skills/SKILL.md",
            install_rel_path: ".mei/skills/meilang-author/SKILL.md",
            summary: "Authoring entrypoint for external AI tools.",
            injection_roles: &["system", "workflow"],
        },
        AssetSeed {
            id: "author_profile",
            topic: "profile",
            kind: "profile",
            title: "Author Profile",
            rel_path: "guides/author-profile.md",
            install_rel_path: ".mei/profiles/author.md",
            summary: "Canonical source-first authoring profile for packaged runtime consumers.",
            injection_roles: &["system", "profile"],
        },
        AssetSeed {
            id: "authoring",
            topic: "workflow",
            kind: "guide",
            title: "Authoring Guide",
            rel_path: "guides/author-skills/authoring.md",
            install_rel_path: ".mei/skills/meilang-author/authoring.md",
            summary: "Source-first authoring workflow and editing boundaries.",
            injection_roles: &["workflow", "repair"],
        },
        AssetSeed {
            id: "syntax_rules",
            topic: "syntax",
            kind: "reference",
            title: "Syntax Rules",
            rel_path: "guides/author-skills/syntax-rules.md",
            install_rel_path: ".mei/skills/meilang-author/syntax-rules.md",
            summary: "Stable MeiLang syntax and current authoring boundaries.",
            injection_roles: &["syntax", "validation"],
        },
        AssetSeed {
            id: "components_reference",
            topic: "components",
            kind: "reference",
            title: "Components Reference",
            rel_path: "guides/author-skills/components-reference.md",
            install_rel_path: ".mei/skills/meilang-author/components-reference.md",
            summary: "Known component ids and usage patterns.",
            injection_roles: &["components", "completion"],
        },
        AssetSeed {
            id: "author_context",
            topic: "context",
            kind: "guide",
            title: "Author Context",
            rel_path: "guides/author-skills/context.md",
            install_rel_path: ".mei/skills/meilang-author/context.md",
            summary: "Reading order and contextual authoring hints.",
            injection_roles: &["workflow", "context"],
        },
        AssetSeed {
            id: "author_runtime_overview",
            topic: "bootstrap",
            kind: "guide",
            title: "Author Runtime Overview",
            rel_path: "knowledge/editor-runtime/authoring-overview.md",
            install_rel_path: ".mei/knowledge/author/authoring-overview.md",
            summary: "Standalone author runtime overview for external tools.",
            injection_roles: &["system", "bootstrap"],
        },
        AssetSeed {
            id: "workflow_recipes",
            topic: "workflow",
            kind: "guide",
            title: "Workflow Recipes",
            rel_path: "knowledge/editor-runtime/workflow-recipes.md",
            install_rel_path: ".mei/knowledge/author/workflow-recipes.md",
            summary: "Recommended authoring recipes for external IDE and agent tools.",
            injection_roles: &["workflow", "recipes"],
        },
        AssetSeed {
            id: "build_debug_ops",
            topic: "ops",
            kind: "guide",
            title: "Build And Debug Ops",
            rel_path: "knowledge/editor-runtime/build-debug-ops.md",
            install_rel_path: ".mei/knowledge/author/build-debug-ops.md",
            summary: "Build, debug, materialize, and doctor command recipes.",
            injection_roles: &["ops", "debug"],
        },
        AssetSeed {
            id: "standalone_minimal_app",
            topic: "examples",
            kind: "example",
            title: "Standalone Minimal App",
            rel_path: "knowledge/editor-runtime/minimal-app-main.mei",
            install_rel_path: ".mei/knowledge/author/minimal-app-main.mei",
            summary: "Minimal standalone MeiLang app entrypoint shipped with the author runtime bundle.",
            injection_roles: &["examples", "bootstrap"],
        },
        AssetSeed {
            id: "standalone_minimal_scene",
            topic: "examples",
            kind: "example",
            title: "Standalone Minimal Home Scene",
            rel_path: "knowledge/editor-runtime/minimal-app-home.mei",
            install_rel_path: ".mei/knowledge/author/minimal-app-home.mei",
            summary: "Minimal standalone MeiLang scene file shipped with the author runtime bundle.",
            injection_roles: &["examples", "bootstrap"],
        },
    ]
}

fn access_assets() -> Vec<AssetSeed> {
    vec![AssetSeed {
        id: "access_profile",
        topic: "profile",
        kind: "guide",
        title: "Access Profile",
        rel_path: "guides/access-profile.md",
        install_rel_path: ".mei/profiles/access.md",
        summary: "World-first access profile guidance for runtime-side tools.",
        injection_roles: &["system", "world"],
    }]
}

fn build_assets(surface: &str, seeds: Vec<AssetSeed>) -> Vec<KnowledgeAssetDescriptor> {
    seeds.into_iter()
        .map(|seed| KnowledgeAssetDescriptor {
            id: seed.id.to_string(),
            surface: surface.to_string(),
            topic: seed.topic.to_string(),
            kind: seed.kind.to_string(),
            title: seed.title.to_string(),
            relative_path: seed.rel_path.to_string(),
            install_relative_path: seed.install_rel_path.to_string(),
            summary: seed.summary.to_string(),
            injection_roles: seed.injection_roles.iter().map(|item| (*item).to_string()).collect(),
        })
        .collect()
}

pub fn knowledge_bundle_descriptor_for_package_root(
    package_root: &Path,
    surface: &str,
) -> Option<KnowledgeBundleDescriptor> {
    let surface = normalize_surface(surface)?;
    let assets = match surface {
        "author" => build_assets(surface, author_assets()),
        "access" => build_assets(surface, access_assets()),
        _ => Vec::new(),
    };
    let primary_entry_ids = assets
        .iter()
        .filter(|asset| asset.kind == "skill_entry" || asset.kind == "profile")
        .map(|asset| asset.id.clone())
        .collect::<Vec<_>>();
    let mut available_topics = assets
        .iter()
        .map(|asset| asset.topic.clone())
        .collect::<Vec<_>>();
    available_topics.sort();
    available_topics.dedup();
    Some(KnowledgeBundleDescriptor {
        schema_version: KNOWLEDGE_BUNDLE_SCHEMA_VERSION.to_string(),
        bundle_id: format!("meilang-{surface}-knowledge"),
        surface: surface.to_string(),
        package_root: package_root.display().to_string(),
        install_dir_rel: ".mei".to_string(),
        primary_entry_ids,
        available_topics,
        assets,
    })
}

fn export_asset(
    package_root: &Path,
    asset: &KnowledgeAssetDescriptor,
    include_content: bool,
) -> Result<KnowledgeAssetContent> {
    let abs_path = package_root.join(&asset.relative_path);
    let content = if include_content {
        if abs_path.is_file() {
            Some(
                fs::read_to_string(&abs_path)
                    .with_context(|| format!("failed to read {}", abs_path.display()))?,
            )
        } else {
            None
        }
    } else {
        None
    };
    Ok(KnowledgeAssetContent {
        descriptor: asset.clone(),
        content,
    })
}

fn export_asset_from_workspace(
    workspace_root: &Path,
    asset: &KnowledgeAssetDescriptor,
    include_content: bool,
) -> Result<KnowledgeAssetContent> {
    let install_path = workspace_root.join(&asset.install_relative_path);
    let content = if include_content {
        if !install_path.is_file() {
            anyhow::bail!(
                "missing workspace knowledge asset `{}` at {}; run `mei-toolchain workspace runtime install --source-root {}` first",
                asset.id,
                install_path.display(),
                workspace_root.display()
            );
        }
        Some(
            fs::read_to_string(&install_path)
                .with_context(|| format!("failed to read {}", install_path.display()))?,
        )
    } else {
        None
    };
    Ok(KnowledgeAssetContent {
        descriptor: asset.clone(),
        content,
    })
}

pub fn export_knowledge_bundle_for_package_root(
    package_root: &Path,
    surface: &str,
    topic: Option<&str>,
    include_content: bool,
) -> Result<serde_json::Value> {
    let descriptor = knowledge_bundle_descriptor_for_package_root(package_root, surface)
        .ok_or_else(|| anyhow::anyhow!("unsupported knowledge surface `{surface}`"))?;
    let topic = topic
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    let selected = descriptor
        .assets
        .iter()
        .filter(|asset| match topic.as_deref() {
            Some(topic_id) => {
                asset.id == topic_id || asset.kind == topic_id || asset.topic == topic_id
            }
            None => true,
        })
        .map(|asset| export_asset(package_root, asset, include_content))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "schema_version": KNOWLEDGE_BUNDLE_SCHEMA_VERSION,
        "descriptor": descriptor,
        "topic": topic,
        "include_content": include_content,
        "assets": selected,
    }))
}

pub fn export_knowledge_bundle_for_workspace_root(
    workspace_root: &Path,
    package_root: &Path,
    surface: &str,
    topic: Option<&str>,
    include_content: bool,
) -> Result<serde_json::Value> {
    let descriptor = knowledge_bundle_descriptor_for_package_root(package_root, surface)
        .ok_or_else(|| anyhow::anyhow!("unsupported knowledge surface `{surface}`"))?;
    let topic = topic
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    let selected = descriptor
        .assets
        .iter()
        .filter(|asset| match topic.as_deref() {
            Some(topic_id) => {
                asset.id == topic_id || asset.kind == topic_id || asset.topic == topic_id
            }
            None => true,
        })
        .map(|asset| export_asset_from_workspace(workspace_root, asset, include_content))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "schema_version": KNOWLEDGE_BUNDLE_SCHEMA_VERSION,
        "workspace_root": workspace_root.display().to_string(),
        "descriptor": descriptor,
        "topic": topic,
        "include_content": include_content,
        "assets": selected,
    }))
}
