use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use mei_lang_kernel::{
    describe_dsl_with_helpers, load_component_assets, resolve_authoring_helpers,
};
use mei_lang_toolchain::resolve_components_root;
use tokio::sync::Mutex;
use tower_lsp::{
    async_trait,
    lsp_types::{
        CompletionItem, CompletionItemKind, CompletionOptions, CompletionParams,
        CompletionResponse, Diagnostic, DidChangeTextDocumentParams,
        DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
        DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse,
        Hover, HoverContents, HoverParams, HoverProviderCapability, InitializeParams,
        InitializeResult, InitializedParams, MarkupContent, MarkupKind, MessageType, OneOf, ServerCapabilities, ServerInfo,
        TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
        TextDocumentSyncSaveOptions, Url,
    },
    Client, LanguageServer,
};


use crate::diagnostics::*;
use crate::source_index;

#[derive(Default)]
struct DiagnosticState {
    published_by_app: HashMap<PathBuf, HashSet<Url>>,
    documents: HashMap<Url, String>,
}

pub struct Backend {
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
    pub fn new(client: Client) -> Self {
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

