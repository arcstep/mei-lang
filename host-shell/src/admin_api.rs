//! Admin Platform HTTP API (0547–0549): resources + config-record + asset-slot + command-job.

use axum::{
    extract::{Extension, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use mei_host_auth::AuthPrincipal;
use mei_lang_kernel::{
    get_asset_slot, get_command_job, get_config_path_record, get_config_record, list_asset_slots,
    put_config_path_record, put_config_record, replace_asset_slot, resolve_app_root,
    run_import_job, AdminApplyPolicy, AdminProviderKind, AdminRecordError, AdminTemplate,
    AdminUiSurface,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::admin_registry::AdminRegistry;
use crate::landing::{discover_workspace_apps, enrich_discovered_apps};
use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub struct AdminResourcesQuery {
    pub app_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRecordGetQuery {
    pub app_id: String,
    pub resource_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRecordPutBody {
    pub app_id: String,
    pub resource_id: String,
    pub revision: u64,
    pub idempotency_key: String,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminErrorBody {
    kind: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_revision: Option<u64>,
}

fn admin_err(status: StatusCode, kind: &'static str, message: impl Into<String>) -> ResponseTuple {
    (
        status,
        Json(AdminErrorBody {
            kind,
            message: message.into(),
            current_revision: None,
        }),
    )
}

fn admin_conflict(current_revision: u64, message: impl Into<String>) -> ResponseTuple {
    (
        StatusCode::CONFLICT,
        Json(AdminErrorBody {
            kind: "conflict",
            message: message.into(),
            current_revision: Some(current_revision),
        }),
    )
}

type ResponseTuple = (StatusCode, Json<AdminErrorBody>);

fn ensure_registry(state: &SharedState) -> AdminRegistrySnapshot {
    let (workspace_root, registry) = {
        let guard = state.read().expect("state lock");
        (
            guard.ctx.workspace_root.clone(),
            guard.admin_registry.clone(),
        )
    };
    let topbar_menu = mei_lang_app::load_topbar_menu_context(workspace_root.as_path());
    let discovered = discover_workspace_apps(workspace_root.as_path()).unwrap_or_default();
    let apps = enrich_discovered_apps(discovered.as_slice(), &topbar_menu);
    registry.refresh_workspace(workspace_root.as_path(), &apps);
    AdminRegistrySnapshot {
        workspace_root,
        registry,
    }
}

struct AdminRegistrySnapshot {
    workspace_root: std::path::PathBuf,
    registry: std::sync::Arc<AdminRegistry>,
}

fn actor_from_principal(principal: Option<&AuthPrincipal>) -> String {
    principal
        .map(|p| format!("{}:{}", p.username, p.role.as_str()))
        .unwrap_or_else(|| "anonymous".to_string())
}

fn principal_has_cap(principal: Option<&AuthPrincipal>, cap: &str) -> bool {
    let Some(p) = principal else {
        // Auth disabled / anonymous: allow config_upload-class admin for local serve.
        return matches!(cap, "config_upload" | "access_view");
    };
    let caps = p.capabilities();
    match cap {
        "config_upload" => caps.config_upload,
        "build_view" => caps.build_view,
        "access_view" => caps.access_view,
        _ => false,
    }
}

pub async fn api_admin_resources(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<AdminResourcesQuery>,
) -> impl IntoResponse {
    let snap = ensure_registry(&state);
    let Some(app_id) = query
        .app_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "validation",
            "app_id query is required",
        )
        .into_response();
    };
    let principal_ref = principal.as_ref().map(|e| &e.0);
    if let Some(p) = principal_ref {
        if !p.can_access_app(app_id) {
            return admin_err(StatusCode::FORBIDDEN, "forbidden", "app not allowed")
                .into_response();
        }
    }
    let resources = snap
        .registry
        .nav_items_for_capabilities(app_id, &|cap| principal_has_cap(principal_ref, cap));
    let diagnostics = snap
        .registry
        .diagnostics()
        .into_iter()
        .filter(|d| d.app_id == app_id)
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(json!({
            "appId": app_id,
            "resources": resources,
            "diagnostics": diagnostics,
        })),
    )
        .into_response()
}

pub async fn api_config_record_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<ConfigRecordGetQuery>,
) -> impl IntoResponse {
    let snap = ensure_registry(&state);
    let principal_ref = principal.as_ref().map(|e| &e.0);
    match load_config_record_context(&snap, principal_ref, &query.app_id, &query.resource_id) {
        Ok((resource, app_root)) => {
            let record = if let Some(config_path) = resource.config_path.as_deref() {
                get_config_path_record(&app_root, config_path)
            } else {
                get_config_record(
                    &app_root,
                    resource.record_path.as_deref().unwrap_or_default(),
                )
            };
            match record {
                Ok(record) => (
                    StatusCode::OK,
                    Json(json!({
                        "resourceId": resource.resource_id,
                        "resourceKey": resource.resource_key,
                        "appId": query.app_id,
                        "scope": "app",
                        "revision": record.revision,
                        "persistedRevision": record.revision,
                        "effectiveRevision": record.revision,
                        "applyPolicy": resource.spec.apply_policy,
                        "payload": record.data,
                        "spec": resource.spec,
                    })),
                )
                    .into_response(),
                Err(e) => map_record_error(e).into_response(),
            }
        }
        Err(resp) => resp.into_response(),
    }
}

pub async fn api_config_record_put(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Json(body): Json<ConfigRecordPutBody>,
) -> impl IntoResponse {
    if body.idempotency_key.trim().is_empty() {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "validation",
            "idempotencyKey is required",
        )
        .into_response();
    }
    let snap = ensure_registry(&state);
    let principal_ref = principal.as_ref().map(|e| &e.0);
    let actor = actor_from_principal(principal_ref);
    let correlation_id = format!(
        "admin-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        body.idempotency_key.chars().take(8).collect::<String>()
    );
    match load_config_record_context(&snap, principal_ref, &body.app_id, &body.resource_id) {
        Ok((resource, app_root)) => {
            let update = if let Some(config_path) = resource.config_path.as_deref() {
                put_config_path_record(
                    &app_root,
                    config_path,
                    body.revision,
                    body.payload,
                    &actor,
                    &body.app_id,
                    &body.resource_id,
                    &correlation_id,
                )
            } else if let Some(record_path) = resource
                .record_path
                .as_deref()
                .filter(|path| !path.is_empty())
            {
                put_config_record(
                    &app_root,
                    record_path,
                    body.revision,
                    body.payload,
                    &actor,
                    &body.app_id,
                    &body.resource_id,
                    &correlation_id,
                )
            } else {
                return admin_err(
                    StatusCode::BAD_REQUEST,
                    "validation",
                    "resource missing record_path or config_path",
                )
                .into_response();
            };
            match update {
                Ok(record) => {
                    let restart_required = matches!(
                        resource.spec.apply_policy,
                        Some(AdminApplyPolicy::RestartRuntime)
                    );
                    if resource.config_path.is_some()
                        && matches!(
                            resource.spec.apply_policy,
                            Some(AdminApplyPolicy::Hot | AdminApplyPolicy::ReloadView)
                        )
                    {
                        crate::access_page_cache::clear_legacy_page_render_cache_for_app(
                            snap.workspace_root.as_path(),
                            &body.app_id,
                        );
                    }
                    let effective_revision = if restart_required {
                        body.revision
                    } else {
                        record.revision
                    };
                    (
                        StatusCode::OK,
                        Json(json!({
                            "ok": true,
                            "resourceId": resource.resource_id,
                            "resourceKey": resource.resource_key,
                            "appId": body.app_id,
                            "scope": "app",
                            "revision": record.revision,
                            "persistedRevision": record.revision,
                            "effectiveRevision": effective_revision,
                            "applyPolicy": resource.spec.apply_policy,
                            "runtimeRestartRequired": restart_required,
                            "payload": record.data,
                            "correlationId": correlation_id,
                            "actor": actor,
                        })),
                    )
                        .into_response()
                }
                Err(e) => map_record_error(e).into_response(),
            }
        }
        Err(resp) => resp.into_response(),
    }
}

fn load_config_record_context(
    snap: &AdminRegistrySnapshot,
    principal: Option<&AuthPrincipal>,
    app_id: &str,
    resource_id: &str,
) -> Result<(mei_lang_kernel::AdminResourceProjection, std::path::PathBuf), axum::response::Response>
{
    if let Some(p) = principal {
        if !p.can_access_app(app_id) {
            return Err(
                admin_err(StatusCode::FORBIDDEN, "forbidden", "app not allowed").into_response(),
            );
        }
    }
    let Some(resource) = snap.registry.resource(app_id, resource_id) else {
        return Err(admin_err(
            StatusCode::NOT_FOUND,
            "not-found",
            format!("resource `{resource_id}` not registered for app `{app_id}`"),
        )
        .into_response());
    };
    for cap in &resource.required_capabilities {
        if !principal_has_cap(principal, cap) {
            return Err(admin_err(
                StatusCode::FORBIDDEN,
                "forbidden",
                format!("missing capability `{cap}`"),
            )
            .into_response());
        }
    }
    if resource.provider != AdminProviderKind::ConfigRecord {
        return Err(admin_err(
            StatusCode::NOT_IMPLEMENTED,
            "provider-unavailable",
            format!(
                "config-record only supports config-record provider; got {:?}",
                resource.provider
            ),
        )
        .into_response());
    }
    if resource.ui_surface != mei_lang_kernel::AdminUiSurface::FormCard {
        return Err(admin_err(
            StatusCode::NOT_IMPLEMENTED,
            "provider-unavailable",
            "Host embed resources do not use config-record API",
        )
        .into_response());
    }
    if resource.template != AdminTemplate::SingletonForm {
        return Err(admin_err(
            StatusCode::NOT_IMPLEMENTED,
            "provider-unavailable",
            "Phase B only supports singleton-form for config-record",
        )
        .into_response());
    }
    if resource
        .record_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .is_none()
        && resource
            .config_path
            .as_deref()
            .filter(|path| !path.is_empty())
            .is_none()
    {
        return Err(admin_err(
            StatusCode::BAD_REQUEST,
            "validation",
            "config-record resource requires record_path or config_path",
        )
        .into_response());
    }
    let app_root = resolve_app_root(snap.workspace_root.as_path(), app_id);
    if !app_root.is_dir() {
        return Err(admin_err(StatusCode::NOT_FOUND, "not-found", "app not found").into_response());
    }
    Ok((resource, app_root))
}

fn map_record_error(err: AdminRecordError) -> ResponseTuple {
    match err {
        AdminRecordError::Conflict { current_revision } => {
            admin_conflict(current_revision, "stale revision")
        }
        AdminRecordError::Validation(msg) => admin_err(StatusCode::BAD_REQUEST, "validation", msg),
        AdminRecordError::NotFound(msg) => admin_err(StatusCode::NOT_FOUND, "not-found", msg),
        AdminRecordError::Parse(msg) | AdminRecordError::Io(msg) => {
            admin_err(StatusCode::INTERNAL_SERVER_ERROR, "internal", msg)
        }
    }
}

fn load_provider_resource(
    snap: &AdminRegistrySnapshot,
    principal: Option<&AuthPrincipal>,
    app_id: &str,
    resource_id: &str,
    expected: AdminProviderKind,
) -> Result<(mei_lang_kernel::AdminResourceProjection, std::path::PathBuf), axum::response::Response>
{
    if let Some(p) = principal {
        if !p.can_access_app(app_id) {
            return Err(
                admin_err(StatusCode::FORBIDDEN, "forbidden", "app not allowed").into_response(),
            );
        }
    }
    let Some(resource) = snap.registry.resource(app_id, resource_id) else {
        return Err(admin_err(
            StatusCode::NOT_FOUND,
            "not-found",
            format!("resource `{resource_id}` not registered for app `{app_id}`"),
        )
        .into_response());
    };
    for cap in &resource.required_capabilities {
        if !principal_has_cap(principal, cap) {
            return Err(admin_err(
                StatusCode::FORBIDDEN,
                "forbidden",
                format!("missing capability `{cap}`"),
            )
            .into_response());
        }
    }
    if resource.provider != expected {
        return Err(admin_err(
            StatusCode::NOT_IMPLEMENTED,
            "provider-unavailable",
            format!("expected {:?}, got {:?}", expected, resource.provider),
        )
        .into_response());
    }
    let app_root = resolve_app_root(snap.workspace_root.as_path(), app_id);
    if !app_root.is_dir() {
        return Err(admin_err(StatusCode::NOT_FOUND, "not-found", "app not found").into_response());
    }
    Ok((resource, app_root))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotQuery {
    pub app_id: String,
    pub resource_id: String,
    pub slot_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotReplaceBody {
    pub app_id: String,
    pub resource_id: String,
    pub slot_id: String,
    pub filename: String,
    pub idempotency_key: String,
    /// UTF-8 text payload (CSV / plain).
    #[serde(default)]
    pub content: Option<String>,
    /// Hex-encoded bytes for binary payloads (xlsx).
    #[serde(default)]
    pub content_hex: Option<String>,
}

fn decode_replace_bytes(body: &AssetSlotReplaceBody) -> Result<Vec<u8>, ResponseTuple> {
    if let Some(hex) = body
        .content_hex
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        return decode_hex(hex)
            .map_err(|msg| admin_err(StatusCode::BAD_REQUEST, "validation", msg));
    }
    if let Some(text) = body.content.as_ref() {
        return Ok(text.as_bytes().to_vec());
    }
    Err(admin_err(
        StatusCode::BAD_REQUEST,
        "validation",
        "content or contentHex is required",
    ))
}

fn decode_hex(hex: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.len() % 2 != 0 {
        return Err("contentHex length must be even".into());
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&cleaned[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn correlation_id(idempotency_key: &str) -> String {
    format!(
        "admin-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
        idempotency_key.chars().take(8).collect::<String>()
    )
}

pub async fn api_asset_slot_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<AssetSlotQuery>,
) -> impl IntoResponse {
    let snap = ensure_registry(&state);
    let principal_ref = principal.as_ref().map(|e| &e.0);
    match load_provider_resource(
        &snap,
        principal_ref,
        &query.app_id,
        &query.resource_id,
        AdminProviderKind::AssetSlot,
    ) {
        Ok((resource, app_root)) => {
            if resource.ui_surface != AdminUiSurface::AssetSlotCollection
                && resource.template != AdminTemplate::AssetSlotCollection
            {
                return admin_err(
                    StatusCode::BAD_REQUEST,
                    "validation",
                    "resource is not an asset-slot-collection",
                )
                .into_response();
            }
            if let Some(slot_id) = query
                .slot_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                match get_asset_slot(&app_root, &resource.spec, slot_id) {
                    Ok(slot) => (StatusCode::OK, Json(json!({ "slot": slot }))).into_response(),
                    Err(e) => map_record_error(e).into_response(),
                }
            } else {
                match list_asset_slots(&app_root, &resource.spec) {
                    Ok(slots) => (StatusCode::OK, Json(json!({ "slots": slots }))).into_response(),
                    Err(e) => map_record_error(e).into_response(),
                }
            }
        }
        Err(resp) => resp.into_response(),
    }
}

pub async fn api_asset_slot_replace(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Json(body): Json<AssetSlotReplaceBody>,
) -> impl IntoResponse {
    if body.idempotency_key.trim().is_empty() {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "validation",
            "idempotencyKey is required",
        )
        .into_response();
    }
    let bytes = match decode_replace_bytes(&body) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };
    let snap = ensure_registry(&state);
    let principal_ref = principal.as_ref().map(|e| &e.0);
    let actor = actor_from_principal(principal_ref);
    let correlation_id = correlation_id(&body.idempotency_key);
    match load_provider_resource(
        &snap,
        principal_ref,
        &body.app_id,
        &body.resource_id,
        AdminProviderKind::AssetSlot,
    ) {
        Ok((resource, app_root)) => {
            match replace_asset_slot(
                &app_root,
                &resource.spec,
                &body.slot_id,
                &body.filename,
                &bytes,
                &actor,
                &body.app_id,
                &correlation_id,
            ) {
                Ok(slot) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "slot": slot,
                        "correlationId": correlation_id,
                        "actor": actor,
                    })),
                )
                    .into_response(),
                Err(e) => map_record_error(e).into_response(),
            }
        }
        Err(resp) => resp.into_response(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandJobGetQuery {
    pub app_id: String,
    pub job_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandJobRunBody {
    pub app_id: String,
    pub resource_id: String,
    pub action: String,
    pub slot_id: String,
    pub filename: String,
    pub idempotency_key: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub content_hex: Option<String>,
}

pub async fn api_command_job_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Query(query): Query<CommandJobGetQuery>,
) -> impl IntoResponse {
    let snap = ensure_registry(&state);
    let principal_ref = principal.as_ref().map(|e| &e.0);
    if let Some(p) = principal_ref {
        if !p.can_access_app(&query.app_id) {
            return admin_err(StatusCode::FORBIDDEN, "forbidden", "app not allowed")
                .into_response();
        }
    }
    let app_root = resolve_app_root(snap.workspace_root.as_path(), &query.app_id);
    if !app_root.is_dir() {
        return admin_err(StatusCode::NOT_FOUND, "not-found", "app not found").into_response();
    }
    match get_command_job(&app_root, &query.job_id) {
        Ok(job) => (StatusCode::OK, Json(json!({ "job": job }))).into_response(),
        Err(e) => map_record_error(e).into_response(),
    }
}

pub async fn api_command_job_run(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    Json(body): Json<CommandJobRunBody>,
) -> impl IntoResponse {
    if body.action.trim() != "import" {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "validation",
            "Phase D only supports action=import",
        )
        .into_response();
    }
    if body.idempotency_key.trim().is_empty() {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "validation",
            "idempotencyKey is required",
        )
        .into_response();
    }
    let replace_body = AssetSlotReplaceBody {
        app_id: body.app_id.clone(),
        resource_id: body.resource_id.clone(),
        slot_id: body.slot_id.clone(),
        filename: body.filename.clone(),
        idempotency_key: body.idempotency_key.clone(),
        content: body.content.clone(),
        content_hex: body.content_hex.clone(),
    };
    let bytes = match decode_replace_bytes(&replace_body) {
        Ok(b) => b,
        Err(e) => return e.into_response(),
    };
    let snap = ensure_registry(&state);
    let principal_ref = principal.as_ref().map(|e| &e.0);
    let actor = actor_from_principal(principal_ref);
    let correlation_id = correlation_id(&body.idempotency_key);
    match load_provider_resource(
        &snap,
        principal_ref,
        &body.app_id,
        &body.resource_id,
        AdminProviderKind::AssetSlot,
    ) {
        Ok((resource, app_root)) => {
            match run_import_job(
                &app_root,
                &resource.spec,
                &body.app_id,
                &body.slot_id,
                &body.filename,
                &bytes,
                &actor,
                &correlation_id,
            ) {
                Ok(job) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "job": job,
                        "correlationId": correlation_id,
                        "actor": actor,
                    })),
                )
                    .into_response(),
                Err(e) => {
                    // Job file may still exist as failed; surface validation errors clearly.
                    map_record_error(e).into_response()
                }
            }
        }
        Err(resp) => resp.into_response(),
    }
}
