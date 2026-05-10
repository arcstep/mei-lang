use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header::CONTENT_TYPE, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use clap::{Parser, Subcommand};
use mei_lang_app::{render_page, UiRouteMode};
use mei_lang_kernel::{
    compile_app, discover_apps, initial_runtime_state, project_runtime_view, read_source_file,
    render_runtime_html, runtime_step, RuntimeIntent, RuntimeState, RuntimeTraceItem,
};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;

#[derive(Parser)]
#[command(name = "mei")]
#[command(about = "MeiLang skeleton server", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve(ServeArgs),
}

#[derive(clap::Args)]
struct ServeArgs {
    #[arg(long, default_value = "examples")]
    source_root: PathBuf,
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[derive(Clone)]
struct AppState {
    source_root: Arc<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct AppQuery {
    target: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SimStepRequest {
    #[serde(default)]
    state: Option<RuntimeState>,
    intent: RuntimeIntent,
}

#[derive(Debug, Serialize)]
struct SimStepResponse {
    state: RuntimeState,
    scene_view: mei_lang_kernel::RuntimeSceneView,
    #[serde(default)]
    trace_delta: Vec<RuntimeTraceItem>,
    html: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Serve(args) => serve(args).await,
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    let source_root = if args.source_root.is_absolute() {
        args.source_root
    } else {
        std::env::current_dir()
            .context("failed to read current directory")?
            .join(args.source_root)
    };
    let state = AppState {
        source_root: Arc::new(source_root),
    };
    let app = Router::new()
        .route("/", get(index))
        .route("/apps/:mode/:app_id", get(app_page))
        .route("/api/projection/:app_id", get(projection_api))
        .route("/api/sim/step/:app_id", post(sim_step_api))
        .route("/workspace-components/*path", get(component_asset))
        .with_state(state)
        .layer(TraceLayer::new_for_http());
    let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
    tracing::info!("serving MeiLang skeleton at http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index(State(state): State<AppState>) -> Result<Redirect, AppError> {
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let first = apps
        .first()
        .ok_or_else(|| AppError::msg("examples source root does not contain any apps"))?;
    Ok(Redirect::to(&format!("/apps/manage/{}", first.id)))
}

async fn app_page(
    State(state): State<AppState>,
    AxumPath((mode, app_id)): AxumPath<(String, String)>,
    Query(query): Query<AppQuery>,
) -> Result<Html<String>, AppError> {
    let apps = discover_apps(&state.source_root).map_err(AppError::from)?;
    let compiled = compile_app(&state.source_root, &app_id).map_err(AppError::from)?;
    let target = query.target.unwrap_or_else(|| compiled.entry_target.clone());
    let source_path = state.source_root.join(&app_id).join(&target);
    let source = read_source_file(&source_path).unwrap_or_else(|_| "".to_string());
    let html = render_page(
        &apps,
        &compiled,
        UiRouteMode::from_slug(&mode),
        Some(target.as_str()),
        Some(source.as_str()),
    );
    Ok(Html(html))
}

async fn projection_api(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
) -> Result<Json<mei_lang_kernel::CompiledApp>, AppError> {
    let compiled = compile_app(&state.source_root, &app_id).map_err(AppError::from)?;
    Ok(Json(compiled))
}

async fn sim_step_api(
    State(state): State<AppState>,
    AxumPath(app_id): AxumPath<String>,
    Json(request): Json<SimStepRequest>,
) -> Result<Json<SimStepResponse>, AppError> {
    let compiled = compile_app(&state.source_root, &app_id).map_err(AppError::from)?;
    let contract = compiled
        .scene_contract
        .ok_or_else(|| AppError::msg(format!("app `{app_id}` does not provide a scene contract")))?;
    let current_state = request
        .state
        .clone()
        .unwrap_or_else(|| initial_runtime_state(&contract, 1));
    let next_state = runtime_step(&contract, request.state, &request.intent);
    let trace_delta = if next_state.trace_events.len() > current_state.trace_events.len() {
        next_state.trace_events[current_state.trace_events.len()..].to_vec()
    } else if request.intent.kind == "sync" {
        next_state.trace_events.clone()
    } else {
        Vec::new()
    };
    let scene_view = project_runtime_view(&contract, &next_state);
    let html = render_runtime_html(&scene_view, &next_state);
    Ok(Json(SimStepResponse {
        state: next_state,
        scene_view,
        trace_delta,
        html,
    }))
}

async fn component_asset(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, AppError> {
    let asset_path = state.source_root.join("_components").join(&path);
    if !asset_path.exists() {
        return Err(AppError::status(
            StatusCode::NOT_FOUND,
            format!("component asset not found: {}", asset_path.display()),
        ));
    }
    let bytes = fs::read(&asset_path)
        .with_context(|| format!("failed to read {}", asset_path.display()))
        .map_err(AppError::from)?;
    let mut response = Response::new(bytes.into());
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_path(&asset_path)),
    );
    Ok(response)
}

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        _ => "text/plain; charset=utf-8",
    }
}

#[derive(Debug)]
struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    fn msg(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    fn status(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::msg(value.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}
