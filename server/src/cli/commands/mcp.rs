use anyhow::Result;
use mei_lang_toolchain::mcp_surface_descriptor;

use super::super::args::{McpArgs, McpCommand, McpDescribeArgs};
use super::super::util::print_json_output;

pub fn mcp_command(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommand::Describe(args) => mcp_describe_command(args),
    }
}

pub fn mcp_describe_command(args: McpDescribeArgs) -> Result<()> {
    let surface = args.surface.trim().to_ascii_lowercase();
    let descriptor = mcp_surface_descriptor(surface.as_str())
        .ok_or_else(|| anyhow::anyhow!("unsupported MCP surface `{surface}`"))?;
    print_json_output(&descriptor, args.json)
}
