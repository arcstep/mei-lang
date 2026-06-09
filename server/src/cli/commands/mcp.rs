use anyhow::Result;
use mei_lang_toolchain::{capability_catalog_descriptor_for_package_root, mcp_surface_descriptor};

use super::super::args::{McpArgs, McpCatalogArgs, McpCommand, McpDescribeArgs};
use super::super::util::{print_json_output, resolve_package_root};

pub fn mcp_command(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommand::Describe(args) => mcp_describe_command(args),
        McpCommand::Catalog(args) => mcp_catalog_command(args),
    }
}

pub fn mcp_describe_command(args: McpDescribeArgs) -> Result<()> {
    let surface = args.surface.trim().to_ascii_lowercase();
    let descriptor = mcp_surface_descriptor(surface.as_str())
        .ok_or_else(|| anyhow::anyhow!("unsupported MCP surface `{surface}`"))?;
    print_json_output(&descriptor, args.json)
}

pub fn mcp_catalog_command(args: McpCatalogArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let descriptor = capability_catalog_descriptor_for_package_root(&package_root);
    print_json_output(&descriptor, args.json)
}
