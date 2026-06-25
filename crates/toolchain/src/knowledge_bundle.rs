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

pub(crate) fn package_root_hint(package_root: &Path) -> String {
    let leaf = package_root
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("mei-package");
    if package_root.join("Cargo.toml").is_file() {
        format!("source-tree:{leaf}")
    } else if package_root.ends_with(Path::new("share/mei")) {
        "installed-layout:share/mei".to_string()
    } else {
        format!("package-layout:{leaf}")
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
            install_rel_path: "runtime/platform/skills/meilang-author/SKILL.md",
            summary: "Authoring entrypoint for external AI tools.",
            injection_roles: &["system", "workflow"],
        },
        AssetSeed {
            id: "author_profile",
            topic: "profile",
            kind: "profile",
            title: "Author Profile",
            rel_path: "guides/author-profile.md",
            install_rel_path: "runtime/platform/profiles/author.md",
            summary: "Canonical source-first authoring profile for packaged runtime consumers.",
            injection_roles: &["system", "profile"],
        },
        AssetSeed {
            id: "authoring",
            topic: "workflow",
            kind: "guide",
            title: "Authoring Guide",
            rel_path: "guides/author-skills/authoring.md",
            install_rel_path: "runtime/platform/skills/meilang-author/authoring.md",
            summary: "Source-first authoring workflow and editing boundaries.",
            injection_roles: &["workflow", "repair"],
        },
        AssetSeed {
            id: "syntax_rules",
            topic: "syntax",
            kind: "reference",
            title: "Syntax Rules",
            rel_path: "guides/author-skills/syntax-rules.md",
            install_rel_path: "runtime/platform/skills/meilang-author/syntax-rules.md",
            summary: "Stable MeiLang syntax and current authoring boundaries.",
            injection_roles: &["syntax", "validation"],
        },
        AssetSeed {
            id: "dsl_reference",
            topic: "syntax",
            kind: "reference",
            title: "DSL Reference",
            rel_path: "guides/author-skills/dsl-reference.md",
            install_rel_path: "runtime/platform/skills/meilang-author/dsl-reference.md",
            summary: "Current app, scene, dataset, chart, and template authoring skeletons.",
            injection_roles: &["syntax", "examples"],
        },
        AssetSeed {
            id: "namespace_reference",
            topic: "syntax",
            kind: "reference",
            title: "Namespace Reference",
            rel_path: "guides/author-skills/namespace-reference.md",
            install_rel_path: "runtime/platform/skills/meilang-author/namespace-reference.md",
            summary: "Current public helper names, typed refs, and compat boundaries.",
            injection_roles: &["syntax", "names"],
        },
        AssetSeed {
            id: "components_reference",
            topic: "components",
            kind: "reference",
            title: "Components Reference",
            rel_path: "guides/author-skills/components-reference.md",
            install_rel_path: "runtime/platform/skills/meilang-author/components-reference.md",
            summary: "Known component ids and usage patterns.",
            injection_roles: &["components", "completion"],
        },
        AssetSeed {
            id: "component_contracts",
            topic: "components",
            kind: "contract",
            title: "Component Contracts",
            rel_path: "knowledge/editor-runtime/components/component-contracts.json",
            install_rel_path: "runtime/platform/knowledge/author/components/component-contracts.json",
            summary: "Machine-readable public component contract index for standalone authoring.",
            injection_roles: &["components", "contracts"],
        },
        AssetSeed {
            id: "dataset_components_guide",
            topic: "components",
            kind: "guide",
            title: "Dataset Components Guide",
            rel_path: "knowledge/editor-runtime/components/dataset-components.md",
            install_rel_path: "runtime/platform/knowledge/author/components/dataset-components.md",
            summary: "Standalone-friendly public guide for dataset table/filter/summary components.",
            injection_roles: &["components", "recipes"],
        },
        AssetSeed {
            id: "chart_components_guide",
            topic: "components",
            kind: "guide",
            title: "Chart Components Guide",
            rel_path: "knowledge/editor-runtime/components/chart-components.md",
            install_rel_path: "runtime/platform/knowledge/author/components/chart-components.md",
            summary: "Standalone-friendly public guide for chart.* components and mapping usage.",
            injection_roles: &["components", "recipes"],
        },
        AssetSeed {
            id: "cockpit_components_guide",
            topic: "components",
            kind: "guide",
            title: "Cockpit Components Guide",
            rel_path: "knowledge/editor-runtime/components/cockpit-components.md",
            install_rel_path: "runtime/platform/knowledge/author/components/cockpit-components.md",
            summary: "Standalone-friendly public guide for cockpit renderer components.",
            injection_roles: &["components", "recipes"],
        },
        AssetSeed {
            id: "author_context",
            topic: "context",
            kind: "guide",
            title: "Author Context",
            rel_path: "guides/author-skills/context.md",
            install_rel_path: "runtime/platform/skills/meilang-author/context.md",
            summary: "Reading order and contextual authoring hints.",
            injection_roles: &["workflow", "context"],
        },
        AssetSeed {
            id: "author_runtime_overview",
            topic: "bootstrap",
            kind: "guide",
            title: "Author Runtime Overview",
            rel_path: "knowledge/editor-runtime/authoring-overview.md",
            install_rel_path: "runtime/platform/knowledge/author/authoring-overview.md",
            summary: "Standalone author runtime overview for external tools.",
            injection_roles: &["system", "bootstrap"],
        },
        AssetSeed {
            id: "workspace_config_reference",
            topic: "config",
            kind: "guide",
            title: "Workspace Config Reference",
            rel_path: "knowledge/editor-runtime/workspace-config-reference.md",
            install_rel_path: "runtime/platform/knowledge/author/workspace-config-reference.md",
            summary: "Standalone guide for workspace bootstrap, create-app, config layering, upload sources, and theme selection.",
            injection_roles: &["bootstrap", "config"],
        },
        AssetSeed {
            id: "workflow_recipes",
            topic: "workflow",
            kind: "guide",
            title: "Workflow Recipes",
            rel_path: "knowledge/editor-runtime/workflow-recipes.md",
            install_rel_path: "runtime/platform/knowledge/author/workflow-recipes.md",
            summary: "Recommended authoring recipes for external IDE and agent tools.",
            injection_roles: &["workflow", "recipes"],
        },
        AssetSeed {
            id: "build_debug_ops",
            topic: "ops",
            kind: "guide",
            title: "Build And Debug Ops",
            rel_path: "knowledge/editor-runtime/build-debug-ops.md",
            install_rel_path: "runtime/platform/knowledge/author/build-debug-ops.md",
            summary: "Build, debug, materialize, and doctor command recipes.",
            injection_roles: &["ops", "debug"],
        },
        AssetSeed {
            id: "cockpit_template_index",
            topic: "templates",
            kind: "guide",
            title: "Cockpit Template Index",
            rel_path: "knowledge/editor-runtime/templates/cockpit-template-index.md",
            install_rel_path: "runtime/platform/knowledge/author/templates/cockpit-template-index.md",
            summary: "Standalone-friendly public guide for cockpit template shells and paths.",
            injection_roles: &["templates", "recipes"],
        },
        AssetSeed {
            id: "template_contracts",
            topic: "templates",
            kind: "contract",
            title: "Template Contracts",
            rel_path: "knowledge/editor-runtime/templates/template-contracts.json",
            install_rel_path: "runtime/platform/knowledge/author/templates/template-contracts.json",
            summary: "Machine-readable public template contract index for standalone authoring.",
            injection_roles: &["templates", "contracts"],
        },
        AssetSeed {
            id: "dsl_contracts",
            topic: "syntax",
            kind: "contract",
            title: "DSL Contracts",
            rel_path: "knowledge/editor-runtime/syntax/dsl-contracts.json",
            install_rel_path: "runtime/platform/knowledge/author/syntax/dsl-contracts.json",
            summary: "Machine-readable contract index for the current public DSL and config refs.",
            injection_roles: &["syntax", "contracts"],
        },
        AssetSeed {
            id: "author_example_pack",
            topic: "examples",
            kind: "guide",
            title: "Author Example Pack",
            rel_path: "knowledge/editor-runtime/examples/README.md",
            install_rel_path: "runtime/platform/knowledge/author/examples/README.md",
            summary: "Curated standalone authoring example pack with proof points per contract.",
            injection_roles: &["examples", "index"],
        },
        AssetSeed {
            id: "standalone_minimal_app",
            topic: "examples",
            kind: "example",
            title: "Standalone Minimal App",
            rel_path: "knowledge/editor-runtime/minimal-app-main.mei",
            install_rel_path: "runtime/platform/knowledge/author/minimal-app-main.mei",
            summary: "Minimal standalone MeiLang app entrypoint shipped with the author runtime bundle.",
            injection_roles: &["examples", "bootstrap"],
        },
        AssetSeed {
            id: "standalone_minimal_scene",
            topic: "examples",
            kind: "example",
            title: "Standalone Minimal Home Scene",
            rel_path: "knowledge/editor-runtime/minimal-app-home.mei",
            install_rel_path: "runtime/platform/knowledge/author/minimal-app-home.mei",
            summary: "Minimal standalone MeiLang scene file shipped with the author runtime bundle.",
            injection_roles: &["examples", "bootstrap"],
        },
        AssetSeed {
            id: "example_dataset_baseline",
            topic: "examples",
            kind: "example",
            title: "Dataset Baseline Example",
            rel_path: "knowledge/editor-runtime/examples/dataset-baseline.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/dataset-baseline.mei",
            summary: "Minimal dataset/table/summary example for standalone authoring.",
            injection_roles: &["examples", "components"],
        },
        AssetSeed {
            id: "example_filter_reactivity",
            topic: "examples",
            kind: "example",
            title: "Filter Reactivity Example",
            rel_path: "knowledge/editor-runtime/examples/filter-reactivity.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/filter-reactivity.mei",
            summary: "Example proving dataset.filter-bar and shared query_state reactivity.",
            injection_roles: &["examples", "workflow"],
        },
        AssetSeed {
            id: "example_chart_baseline",
            topic: "examples",
            kind: "example",
            title: "Chart Baseline Example",
            rel_path: "knowledge/editor-runtime/examples/chart-baseline.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/chart-baseline.mei",
            summary: "Minimal chart example proving the common chart data+mapping contract.",
            injection_roles: &["examples", "components"],
        },
        AssetSeed {
            id: "example_data_table_runtime",
            topic: "examples",
            kind: "example",
            title: "Cockpit Data Table Runtime Example",
            rel_path: "knowledge/editor-runtime/examples/data-table-runtime.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/data-table-runtime.mei",
            summary: "Example proving cockpit.data-table public props on top of the shared table runtime.",
            injection_roles: &["examples", "components"],
        },
        AssetSeed {
            id: "example_cockpit_panel",
            topic: "examples",
            kind: "example",
            title: "Cockpit Panel Example",
            rel_path: "knowledge/editor-runtime/examples/cockpit-panel.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/cockpit-panel.mei",
            summary: "Example proving cockpit shell reuse via panel_ref template cloning.",
            injection_roles: &["examples", "templates"],
        },
        AssetSeed {
            id: "example_template_clone",
            topic: "examples",
            kind: "example",
            title: "Template Clone Example",
            rel_path: "knowledge/editor-runtime/examples/template-clone.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/template-clone.mei",
            summary: "Example proving metric_card template clone via metric_card_ref.",
            injection_roles: &["examples", "templates"],
        },
        AssetSeed {
            id: "example_multi_scene_app",
            topic: "examples",
            kind: "example",
            title: "Multi Scene App Example",
            rel_path: "knowledge/editor-runtime/examples/multi-scene-app.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/multi-scene-app.mei",
            summary: "App root example proving multi-file scene registration through app_add_scene(scene_ref(...)).",
            injection_roles: &["examples", "workflow"],
        },
        AssetSeed {
            id: "example_multi_scene_home",
            topic: "examples",
            kind: "example",
            title: "Multi Scene Home Support Example",
            rel_path: "knowledge/editor-runtime/examples/multi-scene-home.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/multi-scene-home.mei",
            summary: "Support scene file for the public multi-scene app skeleton.",
            injection_roles: &["examples", "workflow"],
        },
        AssetSeed {
            id: "example_multi_scene_insights",
            topic: "examples",
            kind: "example",
            title: "Multi Scene Insights Support Example",
            rel_path: "knowledge/editor-runtime/examples/multi-scene-insights.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/multi-scene-insights.mei",
            summary: "Second support scene file for the public multi-scene app skeleton.",
            injection_roles: &["examples", "workflow"],
        },
        AssetSeed {
            id: "example_frame_layout_advanced",
            topic: "examples",
            kind: "example",
            title: "Frame Layout Advanced Example",
            rel_path: "knowledge/editor-runtime/examples/frame-layout-advanced.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/frame-layout-advanced.mei",
            summary: "Example proving nested panel.blocks plus cockpit shell composition.",
            injection_roles: &["examples", "templates"],
        },
        AssetSeed {
            id: "example_metric_page_baseline",
            topic: "examples",
            kind: "example",
            title: "Metric Page Baseline Example",
            rel_path: "knowledge/editor-runtime/examples/metric-page-baseline.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/metric-page-baseline.mei",
            summary: "Metric-centric page example combining world.add_metric, summary cards, chart, and table.",
            injection_roles: &["examples", "workflow"],
        },
        AssetSeed {
            id: "example_upload_dataset_baseline",
            topic: "examples",
            kind: "example",
            title: "Upload Dataset Baseline Example",
            rel_path: "knowledge/editor-runtime/examples/upload-dataset-baseline.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/upload-dataset-baseline.mei",
            summary: "Example proving upload-backed dataset binding through source_ref(...) and app-local ops.sources.",
            injection_roles: &["examples", "config"],
        },
        AssetSeed {
            id: "example_sim_baseline",
            topic: "examples",
            kind: "example",
            title: "Sim Baseline Example",
            rel_path: "knowledge/editor-runtime/examples/sim-baseline.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/sim-baseline.mei",
            summary: "Minimal sim.scene example for standalone authoring.",
            injection_roles: &["examples", "components"],
        },
        AssetSeed {
            id: "example_map_baseline",
            topic: "examples",
            kind: "example",
            title: "Map Baseline Example",
            rel_path: "knowledge/editor-runtime/examples/map-baseline.mei",
            install_rel_path: "runtime/platform/knowledge/author/examples/map-baseline.mei",
            summary: "Minimal map.maplibre example proving the public mapSpec contract.",
            injection_roles: &["examples", "components"],
        },
        AssetSeed {
            id: "extension_authoring",
            topic: "extension",
            kind: "guide",
            title: "Extension Authoring Boundary",
            rel_path: "knowledge/editor-runtime/extension-authoring.md",
            install_rel_path: "runtime/platform/knowledge/author/extension-authoring.md",
            summary: "Boundary guide for tasks that leave normal MeiLang authoring and enter extension work.",
            injection_roles: &["workflow", "boundaries"],
        },
    ]
}

fn access_assets() -> Vec<AssetSeed> {
    vec![
        AssetSeed {
            id: "meilang_access_skill",
            topic: "workflow",
            kind: "skill_entry",
            title: "MeiLang Access Skill",
            rel_path: "guides/access-skills/SKILL.md",
            install_rel_path: "runtime/platform/skills/meilang-access/SKILL.md",
            summary: "Access-side entrypoint for standalone or host-bound AI tools.",
            injection_roles: &["system", "workflow"],
        },
        AssetSeed {
            id: "access_profile",
            topic: "profile",
            kind: "profile",
            title: "Access Profile",
            rel_path: "guides/access-profile.md",
            install_rel_path: "runtime/platform/profiles/access.md",
            summary: "World-first access profile guidance for runtime-side tools.",
            injection_roles: &["system", "world"],
        },
        AssetSeed {
            id: "access_workflow",
            topic: "workflow",
            kind: "guide",
            title: "Access Workflow",
            rel_path: "guides/access-skills/workflow.md",
            install_rel_path: "runtime/platform/skills/meilang-access/workflow.md",
            summary: "Companion workflow guide for query-state-aware access questions.",
            injection_roles: &["workflow", "world"],
        },
    ]
}

fn build_assets(surface: &str, seeds: Vec<AssetSeed>) -> Vec<KnowledgeAssetDescriptor> {
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
