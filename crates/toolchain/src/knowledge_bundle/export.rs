use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::json;

use super::seeds_access::access_assets;
use super::seeds_author::author_assets;
use super::types::*;

pub(super) fn build_assets(surface: &str, seeds: Vec<AssetSeed>) -> Vec<KnowledgeAssetDescriptor> {
    seeds
        .into_iter()
        .map(|seed| KnowledgeAssetDescriptor {
            id: seed.id.to_string(),
            surface: surface.to_string(),
            topic: seed.topic.to_string(),
            kind: seed.kind.to_string(),
            title: seed.title.to_string(),
            relative_path: seed.rel_path.to_string(),
            install_relative_path: seed.install_rel_path.to_string(),
            summary: seed.summary.to_string(),
            injection_roles: seed
                .injection_roles
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
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
        package_root: package_root_hint(package_root),
        install_dir_rel: "runtime/platform".to_string(),
        primary_entry_ids,
        available_topics,
        assets,
    })
}

pub(super) fn export_asset(
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

pub(super) fn export_asset_from_workspace(
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
