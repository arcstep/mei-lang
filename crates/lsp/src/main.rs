use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use mei_lang_kernel::{
    compile_app_from_root_with_options, CompileOptions, Diagnostic as MeiDiagnostic,
    Severity as MeiSeverity,
};
use tokio::sync::Mutex;
use tower_lsp::{
    async_trait,
    lsp_types::{
        Diagnostic, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, DidSaveTextDocumentParams, InitializeParams, InitializeResult,
        InitializedParams, MessageType, NumberOrString, Position, Range, ServerCapabilities,
        ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
        TextDocumentSyncSaveOptions, Url,
    },
    Client, LanguageServer, LspService, Server,
};

const SERVER_NAME: &str = "mei-lang-lsp";

#[derive(Default)]
struct DiagnosticState {
    published_by_app: HashMap<PathBuf, HashSet<Url>>,
}

struct Backend {
    client: Client,
    state: Arc<Mutex<DiagnosticState>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(Mutex::new(DiagnosticState::default())),
        }
    }

    async fn validate_uri(&self, uri: Url) {
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
                    )],
                    None,
                )
                .await;
            return;
        };
        let source_root = resolve_source_root_for_assets(&app_root);
        let target = to_preview_target(&app_root, &path).unwrap_or_else(|| "main.mei".to_string());
        let options = CompileOptions {
            scene: None,
            preview_target: Some(target),
        };

        let mut grouped = match compile_app_from_root_with_options(&source_root, &app_root, options)
        {
            Ok(compiled) => group_diagnostics(compiled.diagnostics, &source_root, &app_root, &uri),
            Err(error) => {
                let mut map = HashMap::new();
                map.insert(
                    uri.clone(),
                    vec![compile_failure_diagnostic(
                        "compile_error",
                        error.to_string(),
                    )],
                );
                map
            }
        };
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
                ..ServerCapabilities::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "MeiLang LSP 已启动（diagnostics-only）".to_string(),
            )
            .await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.validate_uri(params.text_document.uri).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.validate_uri(params.text_document.uri).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.validate_uri(params.text_document.uri).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .publish_diagnostics(params.text_document.uri, Vec::new(), None)
            .await;
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

fn group_diagnostics(
    diagnostics: Vec<MeiDiagnostic>,
    source_root: &Path,
    app_root: &Path,
    fallback_uri: &Url,
) -> HashMap<Url, Vec<Diagnostic>> {
    let mut grouped: HashMap<Url, Vec<Diagnostic>> = HashMap::new();
    for diag in diagnostics {
        let uri = resolve_source_uri(diag.source_path.as_deref(), source_root, app_root)
            .unwrap_or_else(|| fallback_uri.clone());
        grouped
            .entry(uri)
            .or_default()
            .push(to_lsp_diagnostic(diag));
    }
    grouped
}

fn resolve_source_uri(
    source_path: Option<&str>,
    source_root: &Path,
    app_root: &Path,
) -> Option<Url> {
    let source_path = source_path?;
    let path = PathBuf::from(source_path);
    let resolved = if path.is_absolute() {
        path
    } else {
        let under_app = app_root.join(&path);
        if under_app.exists() || source_path.ends_with(".mei") {
            under_app
        } else {
            source_root.join(path)
        }
    };
    Url::from_file_path(resolved).ok()
}

fn to_lsp_diagnostic(diag: MeiDiagnostic) -> Diagnostic {
    Diagnostic {
        range: zero_range(),
        severity: Some(map_severity(diag.severity)),
        code: Some(NumberOrString::String(diag.code)),
        source: Some(SERVER_NAME.to_string()),
        message: diag.message,
        ..Diagnostic::default()
    }
}

fn compile_failure_diagnostic(code: &str, message: String) -> Diagnostic {
    Diagnostic {
        range: zero_range(),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(code.to_string())),
        source: Some(SERVER_NAME.to_string()),
        message,
        ..Diagnostic::default()
    }
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
