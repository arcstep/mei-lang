use serde::Serialize;

pub const CAPABILITY_CATALOG_SCHEMA_VERSION: &str = "mei-capability-catalog-v1";
pub const MCP_SURFACE_SCHEMA_VERSION: &str = "mei-mcp-surface-v1";

#[derive(Debug, Clone, Serialize)]
pub struct SkillPackageDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source_dir_rel: String,
    pub install_dir_rel: String,
    pub entry_file: String,
    pub companion_priority: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiProfileDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub context_strategy: String,
    pub authority_chain: Vec<String>,
    pub primary_inputs: Vec<String>,
    pub recommended_flow: Vec<String>,
    pub preferred_surface: String,
    pub knowledge_surface: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_package_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance_file_rel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guidance_bundle_asset_id: Option<String>,
}
