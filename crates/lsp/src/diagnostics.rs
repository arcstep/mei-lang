use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use mei_lang_kernel::{Diagnostic as MeiDiagnostic, Severity as MeiSeverity};
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

pub(crate) fn find_app_root(file: &Path) -> Option<PathBuf> {
    let mut current = if file.is_dir() {
        Some(file)
    } else {
        file.parent()
    };
    while let Some(dir) = current {
        if dir.join("main.mei").is_file() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

/// `compile_app_from_root` 用该根目录加载 `workspaces/_components`；对嵌套 app 需向上找到含 `_components` 的祖先。
pub(crate) fn resolve_source_root_for_assets(app_root: &Path) -> PathBuf {
    let mut current: Option<&Path> = Some(app_root);
    while let Some(dir) = current {
        if dir.join("_components").is_dir() {
            return dir.to_path_buf();
        }
        current = dir.parent();
    }
    app_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| app_root.to_path_buf())
}

pub(crate) fn to_preview_target(app_root: &Path, file: &Path) -> Option<String> {
    file.strip_prefix(app_root)
        .ok()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn app_id_from_roots(source_root: &Path, app_root: &Path) -> String {
    app_root
        .strip_prefix(source_root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| {
            app_root
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_string()
        })
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
    } else {
        let under_app = app_root.join(&path);
        if under_app.exists() || source_path.ends_with(".mei") {
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
        "world" => symbol_kind == "world",
        "frame" => symbol_kind == "frame" || symbol_kind == "panel",
        "resource" => symbol_kind == "resource",
        "dataset" => symbol_kind == "dataset" || symbol_kind == "resource",
        "metric" => symbol_kind == "metric",
        _ => false,
    }
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
        if let Some(symbol) = index
            .symbols
            .iter()
            .find(|symbol| symbol.name == value && definition_matches(symbol.kind, reference_kind))
        {
            let uri = Url::from_file_path(&file).ok()?;
            return Some(Location::new(uri, symbol.selection_range));
        }
    }
    None
}

pub(crate) fn hover_doc_for_word(word: &str) -> Option<&'static str> {
    match word {
        "app" => Some("### `app`\n\nDeclares the application entrypoint, title, and default scene."),
        "scene" => Some("### `scene`\n\nDeclares the scene shell and binds world, flow, and frame."),
        "world" => Some("### `world`\n\nDeclares world resources and business data roots."),
        "flow" => Some("### `flow`\n\nDeclares runtime interaction and state transitions."),
        "frame" => Some("### `frame`\n\nDeclares layout and panel slots for a scene."),
        "component" => Some("### `component`\n\nUses a registered component key from the current component catalog."),
        "scene_ref" => Some("### `scene_ref`\n\nReferences a scene object or a scene file binding."),
        "world_ref" => Some("### `world_ref`\n\nReferences a world object or imported world binding."),
        "frame_ref" => Some("### `frame_ref`\n\nReferences a frame object or imported frame binding."),
        "resource_ref" => Some("### `resource_ref`\n\nReferences a resource id from the current world scope."),
        "dataset_ref" => Some("### `dataset_ref`\n\nReferences a dataset or dataset view id from the current world scope."),
        "metric_ref" => Some("### `metric_ref`\n\nReferences a runtime metric definition from the current world scope."),
        "scene_file_ref" => Some("### `scene_file_ref`\n\nReferences an external scene file from the app root."),
        _ => None,
    }
}
