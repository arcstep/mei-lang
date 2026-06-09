use anyhow::Result;
use mei_lang_toolchain::{
    export_knowledge_bundle_for_package_root, export_knowledge_bundle_for_workspace_root,
};

use super::super::args::KnowledgeArgs;
use super::super::util::{
    print_json_output, resolve_optional_cli_source_root, resolve_package_root,
};

pub fn knowledge_command(args: KnowledgeArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let source_root = resolve_optional_cli_source_root(&package_root, args.source_root.as_ref())?;
    let bundle = if let Some(source_root) = source_root {
        export_knowledge_bundle_for_workspace_root(
            &source_root,
            &package_root,
            args.surface.as_str(),
            args.topic.as_deref(),
            args.include_content,
        )?
    } else {
        export_knowledge_bundle_for_package_root(
            &package_root,
            args.surface.as_str(),
            args.topic.as_deref(),
            args.include_content,
        )?
    };
    print_json_output(&bundle, args.json)
}
