use super::prelude::*;
use super::*;

pub(crate) fn write_file(
    path: &Path,
    content: &str,
    force: bool,
) -> Result<EditorRuntimeScaffoldFile> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create scaffold dir {}", parent.display()))?;
    }
    let existed = path.exists();
    if existed && !force {
        return Ok(EditorRuntimeScaffoldFile {
            rel_path: path.to_string_lossy().to_string(),
            overwritten: false,
        });
    }
    fs::write(path, content).with_context(|| format!("write scaffold file {}", path.display()))?;
    Ok(EditorRuntimeScaffoldFile {
        rel_path: path.to_string_lossy().to_string(),
        overwritten: existed,
    })
}

pub(crate) fn write_executable_file(
    path: &Path,
    content: &str,
    force: bool,
) -> Result<EditorRuntimeScaffoldFile> {
    let report = write_file(path, content, force)?;
    set_executable_permissions(path)?;
    Ok(report)
}

pub(crate) fn set_executable_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    if path.is_file() {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .with_context(|| format!("read permissions for {}", path.display()))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .with_context(|| format!("set executable permissions for {}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn copy_runtime_binary(
    target_root: &Path,
    source_path: &Path,
    destination_path: &Path,
    force: bool,
) -> Result<EditorRuntimeScaffoldFile> {
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create runtime binary dir {}", parent.display()))?;
    }
    let existed = destination_path.exists();
    if existed && !force {
        return Ok(normalize_scaffold_files(
            target_root,
            vec![EditorRuntimeScaffoldFile {
                rel_path: destination_path.display().to_string(),
                overwritten: false,
            }],
        )
        .into_iter()
        .next()
        .expect("normalized runtime binary"));
    }
    fs::copy(source_path, destination_path).with_context(|| {
        format!(
            "copy runtime binary {} -> {}",
            source_path.display(),
            destination_path.display()
        )
    })?;
    set_executable_permissions(destination_path)?;
    Ok(normalize_scaffold_files(
        target_root,
        vec![EditorRuntimeScaffoldFile {
            rel_path: destination_path.display().to_string(),
            overwritten: existed,
        }],
    )
    .into_iter()
    .next()
    .expect("normalized runtime binary"))
}
