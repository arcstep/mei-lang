use anyhow::Result;
use mei_lang_toolchain::{
    doctor_editor_runtime_for_package_root, editor_runtime_descriptor_for_package_root,
    scaffold_editor_runtime_tooling,
};

use super::super::args::{
    EditorRuntimeArgs, EditorRuntimeCommand, EditorRuntimeDescribeArgs, EditorRuntimeDoctorArgs,
    EditorRuntimeScaffoldArgs,
};
use super::super::util::{print_json_output, resolve_package_root};

pub fn editor_runtime_command(args: EditorRuntimeArgs) -> Result<()> {
    match args.command {
        EditorRuntimeCommand::Describe(args) => editor_runtime_describe_command(args),
        EditorRuntimeCommand::Doctor(args) => editor_runtime_doctor_command(args),
        EditorRuntimeCommand::Scaffold(args) => editor_runtime_scaffold_command(args),
    }
}

fn editor_runtime_describe_command(args: EditorRuntimeDescribeArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let descriptor = editor_runtime_descriptor_for_package_root(&package_root);
    print_json_output(&descriptor, args.json)
}

fn editor_runtime_doctor_command(args: EditorRuntimeDoctorArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let report = doctor_editor_runtime_for_package_root(&package_root);
    print_json_output(&report, args.json)
}

fn editor_runtime_scaffold_command(args: EditorRuntimeScaffoldArgs) -> Result<()> {
    let package_root = resolve_package_root()?;
    let target_root = if args.target_root.is_absolute() {
        args.target_root
    } else {
        std::env::current_dir()?.join(args.target_root)
    };
    let report =
        scaffold_editor_runtime_tooling(&target_root, &package_root, &args.tools, args.force)?;
    print_json_output(&report, args.json)
}
