use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use mei_lang_kernel::{
    canonical_app_source_rel_path, is_v2_app_root, resolve_app_mei_file_path, resolve_apps_root,
    resolve_workspace_source_root_from_app_root, Diagnostic as MeiDiagnostic,
    Severity as MeiSeverity,
};
use mei_lang_toolchain::{
    compile_app_with_cache, platform_asset_catalog_descriptor_for_workspace_root,
    resolve_components_root,
};
use mei_syntax::parse_source;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, Location, NumberOrString, Position, Range, Url,
};

use crate::source_index;

pub(crate) const SERVER_NAME: &str = "mei-lang-lsp";

/// Walk ancestors until a v2 app root is found (`app.toml` / `app.config.json` / `src/main.mei`),
/// or a legacy flat `main.mei` (editor-runtime / catalog scaffolds).
///
/// Never treat nested `…/src/` as the app root when it only hosts `src/main.mei`.
pub(crate) fn find_app_root(file: &Path) -> Option<PathBuf> {
    let mut current = if file.is_dir() {
        Some(file)
    } else {
        file.parent()
    };
    while let Some(dir) = current {
        if is_v2_app_root(dir) {
            return Some(dir.to_path_buf());
        }
        if dir.join("main.mei").is_file() {
            let is_nested_src_dir = dir
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "src");
            if !is_nested_src_dir {
                return Some(dir.to_path_buf());
            }
            // Fall through so parent (`apps/{id}`) can match via `is_v2_app_root`.
        }
        current = dir.parent();
    }
    None
}

/// Workspace `--source-root` for components / compile (prefer `workspace.json` ancestor).
pub(crate) fn resolve_source_root_for_assets(app_root: &Path) -> PathBuf {
    resolve_workspace_source_root_from_app_root(app_root)
}

pub(crate) fn to_preview_target(app_root: &Path, file: &Path) -> Option<String> {
    let rel = file
        .strip_prefix(app_root)
        .ok()?
        .to_string_lossy()
        .replace('\\', "/");
    Some(canonical_app_source_rel_path(&rel))
}

pub(crate) fn app_id_from_roots(source_root: &Path, app_root: &Path) -> String {
    let apps_root = resolve_apps_root(source_root);
    if let Ok(rel) = app_root.strip_prefix(&apps_root) {
        let id = rel.to_string_lossy().replace('\\', "/");
        if !id.is_empty() {
            return id;
        }
    }
    if let Ok(rel) = app_root.strip_prefix(source_root) {
        let text = rel.to_string_lossy().replace('\\', "/");
        if let Some(rest) = text.strip_prefix("apps/") {
            if !rest.is_empty() {
                return rest.to_string();
            }
        }
        if !text.is_empty() {
            return text;
        }
    }
    app_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_string()
}

pub(crate) fn compile_grouped(
    source_root: &Path,
    app_root: &Path,
    current_file: &Path,
    fallback_uri: &Url,
) -> HashMap<Url, Vec<Diagnostic>> {
    let app_id = app_id_from_roots(source_root, app_root);
    let options = mei_lang_kernel::CompileOptions {
        scene: None,
        preview_target: to_preview_target(app_root, current_file),
        ..Default::default()
    };
    let components_root = resolve_components_root(source_root);
    match compile_app_with_cache(
        source_root,
        app_id.as_str(),
        options,
        components_root.as_path(),
    ) {
        Ok(outcome) => group_diagnostics(
            outcome.compiled.diagnostics,
            source_root,
            app_root,
            fallback_uri,
        ),
        Err(error) => {
            let mut map = HashMap::new();
            map.insert(
                fallback_uri.clone(),
                vec![compile_failure_diagnostic(
                    "compile_error",
                    error.error.to_string(),
                    None,
                )],
            );
            map
        }
    }
}

pub(crate) fn group_diagnostics(
    diagnostics: Vec<MeiDiagnostic>,
    source_root: &Path,
    app_root: &Path,
    fallback_uri: &Url,
) -> HashMap<Url, Vec<Diagnostic>> {
    let mut grouped: HashMap<Url, Vec<Diagnostic>> = HashMap::new();
    for diag in diagnostics {
        let resolved_path = resolve_source_path(diag.source_path.as_deref(), source_root, app_root);
        let uri = resolved_path
            .as_deref()
            .and_then(|path| Url::from_file_path(path).ok())
            .unwrap_or_else(|| fallback_uri.clone());
        let source = resolved_path
            .as_deref()
            .and_then(|path| fs::read_to_string(path).ok());
        grouped
            .entry(uri)
            .or_default()
            .push(to_lsp_diagnostic(diag, source.as_deref()));
    }
    grouped
}

pub(crate) fn resolve_source_path(
    source_path: Option<&str>,
    source_root: &Path,
    app_root: &Path,
) -> Option<PathBuf> {
    let source_path = source_path?;
    let path = PathBuf::from(source_path);
    Some(if path.is_absolute() {
        path
    } else if source_path.ends_with(".mei")
        || source_path.starts_with("src/")
        || source_path.contains('/')
    {
        let resolved = resolve_app_mei_file_path(app_root, source_path);
        if resolved.exists() {
            resolved
        } else {
            let under_app = app_root.join(source_path);
            if under_app.exists() {
                under_app
            } else {
                source_root.join(path)
            }
        }
    } else {
        let under_app = app_root.join(&path);
        if under_app.exists() {
            under_app
        } else {
            source_root.join(path)
        }
    })
}

pub(crate) fn to_lsp_diagnostic(diag: MeiDiagnostic, source: Option<&str>) -> Diagnostic {
    Diagnostic {
        range: diagnostic_range_for_source(source, &diag),
        severity: Some(map_severity(diag.severity)),
        code: Some(NumberOrString::String(diag.code)),
        source: Some(SERVER_NAME.to_string()),
        message: diag.message,
        ..Diagnostic::default()
    }
}

pub(crate) fn compile_failure_diagnostic(
    code: &str,
    message: String,
    range: Option<Range>,
) -> Diagnostic {
    Diagnostic {
        range: range.unwrap_or_else(zero_range),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.to_string())),
        source: Some(SERVER_NAME.to_string()),
        message,
        ..Diagnostic::default()
    }
}

pub(crate) fn syntax_only_diagnostics(uri: Url, path: &Path, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match parse_source(source) {
        Ok(_) => {}
        Err(error) => diagnostics.push(compile_failure_diagnostic(
            "parse_error",
            error.to_string(),
            diagnostic_range_from_parse_error(source, &error),
        )),
    }
    let disk = fs::read_to_string(path).ok();
    if disk.as_ref().is_some_and(|text| text != source) {
        diagnostics.push(Diagnostic {
            range: zero_range(),
            severity: Some(DiagnosticSeverity::INFORMATION),
            code: Some(NumberOrString::String("unsaved_buffer".to_string())),
            source: Some(SERVER_NAME.to_string()),
            message: format!(
                "Unsaved buffer for `{}` is using syntax-only diagnostics; save for full app compile diagnostics.",
                uri.path()
            ),
            ..Diagnostic::default()
        });
    }
    diagnostics
}

pub(crate) fn zero_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 0))
}

pub(crate) fn map_severity(severity: MeiSeverity) -> DiagnosticSeverity {
    match severity {
        MeiSeverity::Error => DiagnosticSeverity::ERROR,
        MeiSeverity::Warning => DiagnosticSeverity::WARNING,
        MeiSeverity::Info => DiagnosticSeverity::INFORMATION,
    }
}

pub(crate) fn diagnostic_range_for_source(source: Option<&str>, diag: &MeiDiagnostic) -> Range {
    let Some(source) = source else {
        return zero_range();
    };
    diagnostic_range_from_message(source, &diag.message)
        .or_else(|| first_non_empty_range(source))
        .unwrap_or_else(zero_range)
}

pub(crate) fn diagnostic_range_from_message(source: &str, message: &str) -> Option<Range> {
    let index = source_index::analyze_source(source);
    for token in extract_message_tokens(message) {
        if let Some(symbol) = index.symbols.iter().find(|symbol| symbol.name == token) {
            return Some(symbol.selection_range);
        }
        if let Some(reference) = index
            .references
            .iter()
            .find(|reference| reference.value == token)
        {
            return Some(reference.range);
        }
        if let Some(range) = find_word_range_in_source(source, &token) {
            return Some(range);
        }
    }
    None
}

pub(crate) fn extract_message_tokens(message: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let chars = message.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let quote = chars[index];
        if quote == '`' || quote == '"' || quote == '\'' {
            let start = index + 1;
            let mut end = start;
            while end < chars.len() && chars[end] != quote {
                end += 1;
            }
            if end < chars.len() {
                let token = chars[start..end]
                    .iter()
                    .collect::<String>()
                    .trim()
                    .to_string();
                if token.len() >= 2 && !tokens.contains(&token) {
                    tokens.push(token);
                }
                index = end + 1;
                continue;
            }
        }
        index += 1;
    }
    tokens
}

pub(crate) fn find_word_range_in_source(source: &str, token: &str) -> Option<Range> {
    for (line_index, line) in source.lines().enumerate() {
        if let Some(column) = line.find(token) {
            let start = Position::new(line_index as u32, column as u32);
            let end = Position::new(line_index as u32, (column + token.len()) as u32);
            return Some(Range::new(start, end));
        }
    }
    None
}

pub(crate) fn first_non_empty_range(source: &str) -> Option<Range> {
    source
        .lines()
        .enumerate()
        .find(|(_, line)| !line.trim().is_empty())
        .map(|(line_index, line)| {
            let end = line.trim_end().len().max(1) as u32;
            Range::new(
                Position::new(line_index as u32, 0),
                Position::new(line_index as u32, end),
            )
        })
}

pub(crate) fn diagnostic_range_from_parse_error(
    source: &str,
    error: &mei_syntax::ParseError,
) -> Option<Range> {
    if error.span_end <= error.span_start {
        return extract_range_from_error(source, &error.message);
    }
    let start = offset_to_position(source, error.span_start);
    let end = offset_to_position(source, error.span_end);
    Some(Range::new(start, end))
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let clamped = offset.min(source.len());
    let mut line = 0u32;
    let mut col = 0u32;
    for ch in source.chars().take(clamped) {
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    Position::new(line, col)
}

pub(crate) fn extract_range_from_error(source: &str, message: &str) -> Option<Range> {
    let mut digits = message
        .split(':')
        .filter_map(|segment| segment.trim().parse::<u32>().ok());
    let line = digits.next()?;
    let col = digits.next()?;
    let zero_line = line.saturating_sub(1);
    let line_len = source
        .lines()
        .nth(zero_line as usize)
        .map(|line| line.len() as u32)
        .unwrap_or(col);
    Some(Range::new(
        Position::new(zero_line, col.saturating_sub(1)),
        Position::new(zero_line, line_len.min(col + 1)),
    ))
}

pub(crate) fn file_location(path: &Path) -> Option<Location> {
    let uri = Url::from_file_path(path).ok()?;
    Some(Location::new(uri, zero_range()))
}

pub(crate) fn component_pack_id(source_root: &Path, component_id: &str) -> Option<String> {
    let descriptor = platform_asset_catalog_descriptor_for_workspace_root(source_root);
    descriptor
        .component_packs
        .into_iter()
        .find(|pack| pack.component_ids.iter().any(|id| id == component_id))
        .map(|pack| pack.id)
}

pub(crate) fn collect_mei_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_mei_files(path.as_path(), out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("mei") {
            out.push(path);
        }
    }
}

pub(crate) fn definition_matches(symbol_kind: &str, reference_kind: &str) -> bool {
    match reference_kind {
        "scene" | "scene_file" => symbol_kind == "scene",
        "plane" => symbol_kind == "plane",
        "region" => symbol_kind == "region",
        "section" => symbol_kind == "section",
        "world" => symbol_kind == "world",
        "frame" => symbol_kind == "frame" || symbol_kind == "panel",
        "panel" => symbol_kind == "panel",
        "resource" => symbol_kind == "resource",
        "dataset" => symbol_kind == "dataset" || symbol_kind == "resource",
        "metric" => symbol_kind == "metric",
        "theme" => symbol_kind == "theme",
        "assembly" => symbol_kind == "assembly" || symbol_kind == "plane" || symbol_kind == "scene",
        "link" => symbol_kind == "link",
        "object" => symbol_kind == "object",
        _ => false,
    }
}

fn symbol_name_matches(symbol_name: &str, reference_value: &str) -> bool {
    if symbol_name == reference_value {
        return true;
    }
    // MCG keys like `fx-structure/home/t1` ↔ local id `t1`
    reference_value
        .rsplit('/')
        .next()
        .is_some_and(|tail| tail == symbol_name)
        || symbol_name
            .rsplit('/')
            .next()
            .is_some_and(|tail| tail == reference_value)
}

pub(crate) fn find_symbol_definition(
    app_root: &Path,
    current_path: &Path,
    current_source: &str,
    reference_kind: &str,
    value: &str,
) -> Option<Location> {
    let mut files = Vec::new();
    collect_mei_files(app_root, &mut files);
    files.sort();
    for file in files {
        let source = if file == current_path {
            current_source.to_string()
        } else {
            match fs::read_to_string(&file) {
                Ok(source) => source,
                Err(_) => continue,
            }
        };
        let index = source_index::analyze_source(&source);
        if let Some(symbol) = index.symbols.iter().find(|symbol| {
            definition_matches(symbol.kind, reference_kind)
                && symbol_name_matches(&symbol.name, value)
        }) {
            let uri = Url::from_file_path(&file).ok()?;
            return Some(Location::new(uri, symbol.selection_range));
        }
    }
    None
}

pub(crate) fn hover_doc_for_word(word: &str) -> Option<&'static str> {
    match word {
        "scene" => Some(
            "### `scene`\n\nScene shell: viewport, grid, and `planes = [plane_ref(...)]`. Gold entry is `src/scene/home.mei` (app root is `app.toml`, not `main.mei`).",
        ),
        "plane_layout" => Some(
            "### `plane_layout`\n\nDeclares a T0/T1/T2 plane (`tier`, `layout`, `regions`). File: `…/t1/plane.mei` or `plane-{id}.mei`.",
        ),
        "region_layout" => {
            Some("### `region_layout`\n\nDeclares a region inside a plane (`sections` / grid areas).")
        }
        "section_layout" => {
            Some("### `section_layout`\n\nDeclares a section slot; hosts panels / page-plane content.")
        }
        "plane_ref" => Some(
            "### `plane_ref`\n\nReferences a plane by MCG key (e.g. `app/home/t1`) or local id.",
        ),
        "region_ref" => Some("### `region_ref`\n\nReferences a region by MCG key or local id."),
        "section_ref" => Some("### `section_ref`\n\nReferences a section by MCG key or local id."),
        "panel_ref" => Some("### `panel_ref`\n\nReferences a panel / content unit by key."),
        "theme_ref" => Some("### `theme_ref`\n\nReferences a theme pack (e.g. `cockpit`)."),
        "assembly_ref" => {
            Some("### `assembly_ref`\n\nReferences an assembly / drilldown target.")
        }
        "link_ref" | "link_decl" => {
            Some("### `link_decl` / `link_ref`\n\nDeclares or references navigation links.")
        }
        "object_ref" => Some("### `object_ref`\n\nReferences a domain object identity."),
        "metric_ref" => Some("### `metric_ref`\n\nReferences a metric definition from the data scope."),
        "dataset_ref" => Some("### `dataset_ref`\n\nReferences a dataset or dataset view."),
        "viewport" => Some("### `viewport`\n\nScene canvas contract (design size, scale, overflow)."),
        "grid" => Some("### `grid`\n\nLayout grid: columns, rows, areas, gap."),
        "component" => {
            Some("### `component`\n\nUses a registered component key from the current component catalog.")
        }
        // Legacy surface — still recognized in older apps / catalog scaffolds.
        "app" => Some(
            "### `app` (legacy Mei entry)\n\nProduct gold apps use `app.toml` + Stage MDX + `src/scene/home.mei` instead of an `app(...)` / `main.mei` entry.",
        ),
        "world" => Some("### `world` (legacy)\n\nHistorical world resource root; prefer current data / object contracts."),
        "flow" => Some("### `flow` (legacy)\n\nHistorical interaction flow declaration."),
        "frame" => Some("### `frame` (legacy)\n\nHistorical layout frame; gold apps use `plane_layout` / regions."),
        "scene_ref" => Some("### `scene_ref`\n\nReferences a scene object or scene file binding."),
        "world_ref" => Some("### `world_ref` (legacy)\n\nReferences a world binding."),
        "frame_ref" => Some("### `frame_ref` (legacy)\n\nReferences a frame binding."),
        "resource_ref" => Some("### `resource_ref`\n\nReferences a resource id from the current scope."),
        "scene_file_ref" => {
            Some("### `scene_file_ref`\n\nReferences an external scene file under the app (`src/…`).")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mei-lsp-{label}-{nanos}"));
        fs::create_dir_all(&dir).expect("mkdir tmp");
        dir
    }

    #[test]
    fn find_app_root_from_app_toml_scene_tree() {
        let root = unique_tmp("toml");
        let app = root.join("apps/demo");
        let plane = app.join("src/scene/home/t1/plane.mei");
        fs::create_dir_all(plane.parent().unwrap()).expect("mkdir");
        fs::write(app.join("app.toml"), "title = \"demo\"\ndefault_stage = \"home\"\n")
            .expect("toml");
        fs::write(&plane, "plane_layout(id = \"t1\")\n").expect("plane");
        assert_eq!(find_app_root(&plane).as_deref(), Some(app.as_path()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn find_app_root_from_src_main_mei() {
        let root = unique_tmp("src-main");
        let app = root.join("apps/hello");
        let main = app.join("src/main.mei");
        fs::create_dir_all(main.parent().unwrap()).expect("mkdir");
        fs::write(&main, "app(id = \"hello\")\n").expect("main");
        assert_eq!(find_app_root(&main).as_deref(), Some(app.as_path()));
        // Must not treat `src/` as the app root.
        assert_ne!(find_app_root(&main).as_deref(), Some(app.join("src").as_path()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn find_app_root_flat_legacy_main_mei() {
        let root = unique_tmp("flat");
        let app = root.join("demo");
        fs::create_dir_all(&app).expect("mkdir");
        let main = app.join("main.mei");
        fs::write(&main, "app(id = \"demo\")\n").expect("main");
        assert_eq!(find_app_root(&main).as_deref(), Some(app.as_path()));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn find_app_root_missing() {
        let root = unique_tmp("missing");
        let file = root.join("orphan/note.mei");
        fs::create_dir_all(file.parent().unwrap()).expect("mkdir");
        fs::write(&file, "scene(id = \"x\")\n").expect("write");
        assert!(find_app_root(&file).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn app_id_strips_apps_prefix() {
        let root = unique_tmp("app-id");
        fs::write(root.join("workspace.json"), r#"{"schemaVersion":2}"#).expect("ws");
        let app = root.join("apps/fx-structure");
        fs::create_dir_all(&app).expect("mkdir");
        fs::write(app.join("app.toml"), "title = \"fx\"\n").expect("toml");
        let source_root = resolve_source_root_for_assets(&app);
        assert_eq!(app_id_from_roots(&source_root, &app), "fx-structure");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn to_preview_target_keeps_src_prefix() {
        let root = unique_tmp("preview");
        let app = root.join("apps/demo");
        let file = app.join("src/scene/home/t1/plane.mei");
        fs::create_dir_all(file.parent().unwrap()).expect("mkdir");
        fs::write(app.join("app.toml"), "title = \"demo\"\n").expect("toml");
        fs::write(&file, "plane_layout(id = \"t1\")\n").expect("plane");
        assert_eq!(
            to_preview_target(&app, &file).as_deref(),
            Some("src/scene/home/t1/plane.mei")
        );
        let _ = fs::remove_dir_all(&root);
    }
}
