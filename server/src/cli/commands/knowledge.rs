use anyhow::Result;
use mei_lang_toolchain::export_knowledge_bundle_for_package_root;

use super::super::args::KnowledgeArgs;
use super::super::util::{print_json_output, resolve_package_root};

pub fn knowledge_command(args: KnowledgeArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let bundle = export_knowledge_bundle_for_package_root(
        &package_root,
        args.surface.as_str(),
        args.topic.as_deref(),
        args.include_content,
    )?;
    print_json_output(&bundle, args.json)
}
