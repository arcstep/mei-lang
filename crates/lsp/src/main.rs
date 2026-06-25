mod source_index;

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use mei_lang_kernel::{
    describe_dsl_with_helpers, load_component_assets, resolve_authoring_helpers,
    Diagnostic as MeiDiagnostic, Severity as MeiSeverity,
};
use mei_lang_toolchain::{
    compile_app_with_cache, platform_asset_catalog_descriptor_for_workspace_root,
    resolve_components_root,
};
use starlark::syntax::{AstModule, Dialect};
use tokio::sync::Mutex;
use tower_lsp::{
    async_trait,
    lsp_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams,
        DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
        DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse,
        Hover, HoverContents, HoverParams, HoverProviderCapability, InitializeParams,
        InitializeResult, InitializedParams, Location, MarkupContent, MarkupKind, MessageType,
        NumberOrString, OneOf, Position, Range, ServerCapabilities, ServerInfo,
        TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
        TextDocumentSyncSaveOptions, Url,
    },
    Client, LanguageServer, LspService, Server,
};

const SERVER_NAME: &str = "mei-lang-lsp";

#[derive(Default)]
struct DiagnosticState {
    published_by_app: HashMap<PathBuf, HashSet<Url>>,
    documents: HashMap<Url, String>,
}

struct Backend {
    client: Client,
    state: Arc<Mutex<DiagnosticState>>,
}

#[derive(Clone, Copy)]
enum ValidationTrigger {
    Open,
    Change,
    Save,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(Mutex::new(DiagnosticState::default())),
        }
    }

    async fn upsert_document(&self, uri: Url, text: String) {
        let mut guard = self.state.lock().await;
        guard.documents.insert(uri, text);
    }

    async fn remove_document(&self, uri: &Url) {
        let mut guard = self.state.lock().await;
        guard.documents.remove(uri);
    }

    async fn document_text(&self, uri: &Url) -> Option<String> {
        let guard = self.state.lock().await;
        guard.documents.get(uri).cloned()
    }

    async fn validate_uri(&self, uri: Url, trigger: ValidationTrigger) {
        if uri.scheme() != "file" {
            return;
        }
        let path = match uri.to_file_path() {
            Ok(path) => path,
            Err(_) => return,
        };
        if path.extension().and_then(|ext| ext.to_str()) != Some("mei") {
            return;
        }
        let Some(app_root) = find_app_root(&path) else {
            self.client
                .publish_diagnostics(
                    uri.clone(),
                    vec![compile_failure_diagnostic(
                        "app_root_not_found",
                        "未找到 app 根目录（缺少 main.mei）".to_string(),
                        None,
                    )],
                    None,
                )
                .await;
            return;
        };
        let buffer = self.document_text(&uri).await;
        let disk = fs::read_to_string(&path).ok();
        let active_source = buffer.clone().or(disk.clone()).unwrap_or_default();
        let dirty = buffer
            .as_ref()
            .zip(disk.as_ref())
            .is_some_and(|(buffer, disk)| buffer != disk);
        if dirty && matches!(trigger, ValidationTrigger::Change) {
            let mut grouped = HashMap::new();
            grouped.insert(
                uri.clone(),
                syntax_only_diagnostics(uri.clone(), &path, &active_source),
            );
            self.publish_grouped(app_root, grouped).await;
            return;
        }
        let source_root = resolve_source_root_for_assets(&app_root);
        let mut grouped = compile_grouped(&source_root, &app_root, &path, &uri);
        if let Some(current) = grouped.get_mut(&uri) {
            current.extend(syntax_only_diagnostics(uri.clone(), &path, &active_source));
        }
        // 始终刷新当前文件，避免切换后残留旧诊断。
        grouped.entry(uri.clone()).or_default();
        self.publish_grouped(app_root, grouped).await;
    }

    async fn publish_grouped(&self, app_root: PathBuf, grouped: HashMap<Url, Vec<Diagnostic>>) {
        let next_uris: HashSet<Url> = grouped.keys().cloned().collect();
        let previous_uris = {
            let mut guard = self.state.lock().await;
            guard.published_by_app.remove(&app_root).unwrap_or_default()
        };

        for (uri, diagnostics) in grouped {
            self.client
                .publish_diagnostics(uri, diagnostics, None)
                .await;
        }

        for uri in previous_uris.difference(&next_uris) {
            self.client
                .publish_diagnostics(uri.clone(), Vec::new(), None)
                .await;
        }

        let mut guard = self.state.lock().await;
        guard.published_by_app.insert(app_root, next_uris);
    }

    async fn document_source(&self, uri: &Url, path: &Path) -> Option<String> {
        self.document_text(uri)
            .await
            .or_else(|| fs::read_to_string(path).ok())
    }

    async fn source_index_for_uri(
        &self,
        uri: &Url,
    ) -> Option<(PathBuf, PathBuf, PathBuf, source_index::SourceIndex, String)> {
        let path = uri.to_file_path().ok()?;
        let app_root = find_app_root(&path)?;
        let source_root = resolve_source_root_for_assets(&app_root);
        let source = self.document_source(uri, &path).await?;
        let index = source_index::analyze_source(&source);
        Some((path, app_root, source_root, index, source))
    }
}

#[async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: SERVER_NAME.to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                        ..TextDocumentSyncOptions::default()
                    },
                )),
                document_symbol_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        "\"".to_string(),
                        ".".to_string(),
                        "_".to_string(),
                    ]),
                    ..CompletionOptions::default()
                }),
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "MeiLang LSP 已启动（editor runtime）".to_string(),
            )
            .await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.upsert_document(params.text_document.uri.clone(), params.text_document.text)
            .await;
        self.validate_uri(params.text_document.uri, ValidationTrigger::Open)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.upsert_document(params.text_document.uri.clone(), change.text)
                .await;
        }
        self.validate_uri(params.text_document.uri, ValidationTrigger::Change)
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        if let Some(text) = params.text {
            self.upsert_document(params.text_document.uri.clone(), text)
                .await;
        }
        self.validate_uri(params.text_document.uri, ValidationTrigger::Save)
            .await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.remove_document(&params.text_document.uri).await;
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> tower_lsp::jsonrpc::Result<Option<DocumentSymbolResponse>> {
        let Some((_, _, _, index, _)) = self.source_index_for_uri(&params.text_document.uri).await
        else {
            return Ok(None);
        };
        Ok(Some(DocumentSymbolResponse::Nested(
            source_index::document_symbols(&index),
        )))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some((path, app_root, source_root, index, source)) =
            self.source_index_for_uri(&uri).await
        else {
            return Ok(None);
        };
        let Some(reference) = source_index::reference_at_position(&index, position) else {
            return Ok(None);
        };
        if reference.kind == "scene_file" {
            let target = app_root.join(&reference.value);
            if let Some(location) = file_location(target.as_path()) {
                return Ok(Some(GotoDefinitionResponse::Scalar(location)));
            }
        }
        if reference.kind == "component" {
            if let Ok(assets) = load_component_assets(&source_root) {
                if let Some(asset) = assets.get(&reference.value) {
                    let target = resolve_components_root(&source_root).join(&asset.script);
                    if let Some(location) = file_location(target.as_path()) {
                        return Ok(Some(GotoDefinitionResponse::Scalar(location)));
                    }
                }
            }
        }
        if let Some(location) = find_symbol_definition(
            app_root.as_path(),
            path.as_path(),
            &source,
            reference.kind,
            &reference.value,
        ) {
            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }
        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let Some((_, _, source_root, index, source)) = self.source_index_for_uri(&uri).await else {
            return Ok(None);
        };
        if let Some(reference) = source_index::reference_at_position(&index, position) {
            let contents = if reference.kind == "component" {
                if let Ok(assets) = load_component_assets(&source_root) {
                    assets.get(&reference.value).map(|asset| {
                        format!(
                            "### component `{}`\n\n- pack: `{}`\n- tag: `{}`\n- script: `{}`",
                            asset.key,
                            component_pack_id(source_root.as_path(), reference.value.as_str())
                                .unwrap_or_else(|| "unknown".to_string()),
                            asset.tag,
                            asset.script
                        )
                    })
                } else {
                    None
                }
            } else {
                Some(format!(
                    "### {} reference\n\nTarget: `{}`",
                    reference.kind, reference.value
                ))
            };
            if let Some(contents) = contents {
                return Ok(Some(Hover {
                    contents: HoverContents::Markup(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: contents,
                    }),
                    range: Some(reference.range),
                }));
            }
        }
        if let Some(symbol) = source_index::symbol_at_position(&index, position) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("### {}\n\nDeclared `{}`", symbol.detail, symbol.name),
                }),
                range: Some(symbol.selection_range),
            }));
        }
        let Some(word) = source_index::word_at_position(&source, position) else {
            return Ok(None);
        };
        if let Some(message) = hover_doc_for_word(&word) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: message.to_string(),
                }),
                range: None,
            }));
        }
        Ok(None)
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> tower_lsp::jsonrpc::Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let position = params.text_document_position.position;
        let Some((_, _, source_root, _, source)) = self.source_index_for_uri(&uri).await else {
            return Ok(None);
        };
        let line = source.lines().nth(position.line as usize).unwrap_or("");
        let prefix = &line[..line
            .char_indices()
            .nth(position.character as usize)
            .map(|(offset, _)| offset)
            .unwrap_or(line.len())];
        let mut items = Vec::new();
        if prefix.contains("component(") && prefix.matches('"').count() % 2 == 1 {
            if let Ok(assets) = load_component_assets(&source_root) {
                for asset in assets.values() {
                    let pack_id =
                        component_pack_id(source_root.as_path(), asset.key.as_str()).unwrap_or_else(|| "unknown".to_string());
                    items.push(CompletionItem {
                        label: asset.key.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(format!(
                            "pack={} tag={} script={}",
                            pack_id, asset.tag, asset.script
                        )),
                        documentation: Some(tower_lsp::lsp_types::Documentation::MarkupContent(
                            MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: format!(
                                    "Component `{}` from pack `{}` (`{}`)",
                                    asset.key, pack_id, asset.script
                                ),
                            },
                        )),
                        ..CompletionItem::default()
                    });
                }
            }
        } else {
            let helpers = resolve_authoring_helpers(&source_root).ok();
            let dsl = describe_dsl_with_helpers(helpers.as_ref());
            if let Some(surface) = dsl.get("public_surface").and_then(|value| value.as_array()) {
                for item in surface {
                    if let Some(label) = item.as_str() {
                        items.push(CompletionItem {
                            label: label.to_string(),
                            kind: Some(CompletionItemKind::FUNCTION),
                            detail: Some("MeiLang DSL surface".to_string()),
                            documentation: hover_doc_for_word(label).map(|value| {
                                tower_lsp::lsp_types::Documentation::MarkupContent(MarkupContent {
                                    kind: MarkupKind::Markdown,
                                    value: value.to_string(),
                                })
                            }),
                            ..CompletionItem::default()
                        });
                    }
                }
            }
            for label in [
                "scene_ref",
                "world_ref",
                "frame_ref",
                "resource_ref",
                "dataset_ref",
                "metric_ref",
                "scene_file_ref",
            ] {
                items.push(CompletionItem {
                    label: label.to_string(),
                    kind: Some(CompletionItemKind::REFERENCE),
                    detail: Some("Reference helper".to_string()),
                    documentation: hover_doc_for_word(label).map(|value| {
                        tower_lsp::lsp_types::Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: value.to_string(),
                        })
                    }),
                    ..CompletionItem::default()
                });
            }
        }
        Ok(Some(CompletionResponse::Array(items)))
    }
}

fn find_app_root(file: &Path) -> Option<PathBuf> {
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
fn resolve_source_root_for_assets(app_root: &Path) -> PathBuf {
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

fn to_preview_target(app_root: &Path, file: &Path) -> Option<String> {
    file.strip_prefix(app_root)
        .ok()
        .map(|value| value.to_string_lossy().replace('\\', "/"))
}

fn app_id_from_roots(source_root: &Path, app_root: &Path) -> String {
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

fn compile_grouped(
    source_root: &Path,
    app_root: &Path,
    current_file: &Path,
    fallback_uri: &Url,
) -> HashMap<Url, Vec<Diagnostic>> {
    let app_id = app_id_from_roots(source_root, app_root);
    let options = mei_lang_kernel::CompileOptions {
        scene: None,
        preview_target: to_preview_target(app_root, current_file),
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

fn group_diagnostics(
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

fn resolve_source_path(
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

fn to_lsp_diagnostic(diag: MeiDiagnostic, source: Option<&str>) -> Diagnostic {
    Diagnostic {
        range: diagnostic_range_for_source(source, &diag),
        severity: Some(map_severity(diag.severity)),
        code: Some(NumberOrString::String(diag.code)),
        source: Some(SERVER_NAME.to_string()),
        message: diag.message,
        ..Diagnostic::default()
    }
}

fn compile_failure_diagnostic(code: &str, message: String, range: Option<Range>) -> Diagnostic {
    Diagnostic {
        range: range.unwrap_or_else(zero_range),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.to_string())),
        source: Some(SERVER_NAME.to_string()),
        message,
        ..Diagnostic::default()
    }
}

fn syntax_only_diagnostics(uri: Url, path: &Path, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match AstModule::parse(
        path.to_string_lossy().as_ref(),
        source.to_string(),
        &Dialect::Standard,
    ) {
        Ok(_) => {}
        Err(error) => diagnostics.push(compile_failure_diagnostic(
            "parse_error",
            error.to_string(),
            extract_range_from_error(source, &error.to_string()),
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

fn zero_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 0))
}

fn map_severity(severity: MeiSeverity) -> DiagnosticSeverity {
    match severity {
        MeiSeverity::Error => DiagnosticSeverity::ERROR,
        MeiSeverity::Warning => DiagnosticSeverity::WARNING,
        MeiSeverity::Info => DiagnosticSeverity::INFORMATION,
    }
}

fn diagnostic_range_for_source(source: Option<&str>, diag: &MeiDiagnostic) -> Range {
    let Some(source) = source else {
        return zero_range();
    };
    diagnostic_range_from_message(source, &diag.message)
        .or_else(|| first_non_empty_range(source))
        .unwrap_or_else(zero_range)
}

fn diagnostic_range_from_message(source: &str, message: &str) -> Option<Range> {
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

fn extract_message_tokens(message: &str) -> Vec<String> {
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

fn find_word_range_in_source(source: &str, token: &str) -> Option<Range> {
    for (line_index, line) in source.lines().enumerate() {
        if let Some(column) = line.find(token) {
            let start = Position::new(line_index as u32, column as u32);
            let end = Position::new(line_index as u32, (column + token.len()) as u32);
            return Some(Range::new(start, end));
        }
    }
    None
}

fn first_non_empty_range(source: &str) -> Option<Range> {
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

fn extract_range_from_error(source: &str, message: &str) -> Option<Range> {
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

fn file_location(path: &Path) -> Option<Location> {
    let uri = Url::from_file_path(path).ok()?;
    Some(Location::new(uri, zero_range()))
}

fn component_pack_id(source_root: &Path, component_id: &str) -> Option<String> {
    let descriptor = platform_asset_catalog_descriptor_for_workspace_root(source_root);
    descriptor
        .component_packs
        .into_iter()
        .find(|pack| pack.component_ids.iter().any(|id| id == component_id))
        .map(|pack| pack.id)
}

fn collect_mei_files(root: &Path, out: &mut Vec<PathBuf>) {
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

fn definition_matches(symbol_kind: &str, reference_kind: &str) -> bool {
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

fn find_symbol_definition(
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

fn hover_doc_for_word(word: &str) -> Option<&'static str> {
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

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .compact()
        .init();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
