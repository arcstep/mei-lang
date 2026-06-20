use anyhow::Result;

use super::super::args::PrebuildArgs;
use super::super::util::{
    print_json_output, resolve_cli_source_root, resolve_package_root, resolve_source_root_arg,
};
use crate::agent_runtime;
use crate::prebuild::{run_prebuild, PrebuildMode, PrebuildOptions};

pub fn prebuild_command(args: PrebuildArgs) -> Result<()> {
    if args.verify && args.clean {
        anyhow::bail!("`prebuild --verify` and `--clean` cannot be used together");
    }
    let package_root = resolve_package_root()?;
    agent_runtime::runtime::load_repo_dotenv(&package_root);
    let raw_source_root =
        resolve_source_root_arg(&package_root, args.workspace.as_deref(), &args.source_root)?;
    let source_root = resolve_cli_source_root(&package_root, &raw_source_root)?;
    let report = run_prebuild(
        source_root.as_path(),
        &PrebuildOptions {
            app_filter: args
                .app_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            mode: if args.verify {
                PrebuildMode::Verify
            } else {
                PrebuildMode::Build
            },
            clean: args.clean,
        },
    )?;
    print_json_output(&report, args.json)
}
