//! Typed Admin Provider HTTP API.

use axum::{
    body::Body,
    extract::{Extension, Path as AxumPath, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use mei_host_auth::AuthPrincipal;
use mei_lang_kernel::{
    apply_asset_slot_current, delete_asset_slot_file, get_asset_slot, get_command_job,
    get_config_record, put_config_record, replace_asset_slot, resolve_app_root,
    resolve_asset_slot_download_path, run_import_job, AdminEntryProjection, AdminRecordError,
    ProviderBinding,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

use crate::admin_registry::AdminRegistry;
use crate::state::SharedState;

#[derive(Debug, Deserialize)]
pub struct AdminResourcesQuery {
    pub app_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderContext {
    pub app_id: String,
    pub resource_id: String,
    pub module_id: String,
    pub provider_id: String,
    pub method: String,
    pub target: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderGetQuery {
    #[serde(flatten)]
    pub context: ProviderContext,
    pub job_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigRecordPutBody {
    #[serde(flatten)]
    pub context: ProviderContext,
    pub revision: Option<u64>,
    pub idempotency_key: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotReplaceBody {
    #[serde(flatten)]
    pub context: ProviderContext,
    pub filename: String,
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub content_hex: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotApplyBody {
    #[serde(flatten)]
    pub context: ProviderContext,
    pub filename: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotDeleteBody {
    #[serde(flatten)]
    pub context: ProviderContext,
    pub filename: String,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotDownloadBody {
    #[serde(flatten)]
    pub context: ProviderContext,
    pub filename: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetSlotDownloadQuery {
    #[serde(flatten)]
    pub context: ProviderContext,
    pub filename: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandJobRunBody {
    #[serde(flatten)]
    pub context: ProviderContext,
    pub asset_binding_id: String,
    pub filename: String,
    pub idempotency_key: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub content_hex: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminErrorBody {
    kind: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_revision: Option<u64>,
}

type ResponseTuple = (StatusCode, Json<AdminErrorBody>);

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

fn ensure_registry(state: &SharedState) -> AdminRegistrySnapshot {
    let (workspace_root, registry) = {
        let guard = state.read().expect("state lock");
        (
            guard.ctx.workspace_root.clone(),
            guard.admin_registry.clone(),
        )
    };
    AdminRegistrySnapshot {
        workspace_root,
        registry,
    }
}

fn ensure_app_registry(snap: &AdminRegistrySnapshot, app_id: &str) {
    snap.registry
        .ensure_app_loaded(snap.workspace_root.as_path(), app_id);
}

struct AdminRegistrySnapshot {
    workspace_root: std::path::PathBuf,
    registry: std::sync::Arc<AdminRegistry>,
}

type ProviderRoute = (String, String, String);

fn route_matches_context(route: &ProviderRoute, context: &ProviderContext) -> bool {
    route.0 == context.app_id && route.1 == context.resource_id && route.2 == context.module_id
}

fn actor_from_principal(principal: Option<&AuthPrincipal>) -> String {
    principal
        .map(|value| format!("{}:{}", value.username, value.role.as_str()))
        .unwrap_or_else(|| "anonymous".to_string())
}

fn principal_has_cap(principal: Option<&AuthPrincipal>, capability: &str) -> bool {
    let Some(principal) = principal else {
        return matches!(capability, "config_upload" | "access_view");
    };
    let capabilities = principal.capabilities();
    match capability {
        "config_upload" => capabilities.config_upload,
        "build_view" => capabilities.build_view,
        "access_view" => capabilities.access_view,
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
        .filter(|value| !value.is_empty())
    else {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "validation",
            "app_id query is required",
        )
        .into_response();
    };
    let principal = principal.as_ref().map(|value| &value.0);
    if principal.is_some_and(|value| !value.can_access_app(app_id)) {
        return admin_err(StatusCode::FORBIDDEN, "forbidden", "app not allowed").into_response();
    }
    ensure_app_registry(&snap, app_id);
    let resources = snap
        .registry
        .nav_items_for_capabilities(app_id, &|capability| {
            principal_has_cap(principal, capability)
        });
    (
        StatusCode::OK,
        Json(json!({
            "appId": app_id,
            "resources": resources,
            "diagnostics": snap.registry.diagnostics().into_iter()
                .filter(|diagnostic| diagnostic.app_id == app_id).collect::<Vec<_>>(),
        })),
    )
        .into_response()
}

fn load_provider_context(
    snap: &AdminRegistrySnapshot,
    principal: Option<&AuthPrincipal>,
    context: &ProviderContext,
) -> Result<(AdminEntryProjection, ProviderBinding, std::path::PathBuf), axum::response::Response> {
    if principal.is_some_and(|value| !value.can_access_app(context.app_id.as_str())) {
        return Err(
            admin_err(StatusCode::FORBIDDEN, "forbidden", "app not allowed").into_response(),
        );
    }
    ensure_app_registry(snap, context.app_id.as_str());
    let Some(resource) = snap.registry.resource(
        context.app_id.as_str(),
        context.resource_id.as_str(),
        context.module_id.as_str(),
    ) else {
        return Err(admin_err(
            StatusCode::NOT_FOUND,
            "not-found",
            "AdminRegistryEntry not found",
        )
        .into_response());
    };
    let binding = resource
        .page_program
        .provider_bindings
        .iter()
        .find(|binding| {
            binding.provider_id == context.provider_id
                && binding.method == context.method.to_ascii_uppercase()
                && binding.target == context.target
        })
        .cloned()
        .ok_or_else(|| {
            admin_err(
                StatusCode::FORBIDDEN,
                "binding-mismatch",
                "provider, method, or target is not declared by this Admin scene",
            )
            .into_response()
        })?;
    for capability in resource
        .registry_entry
        .required_capabilities
        .iter()
        .chain(binding.required_capabilities.iter())
    {
        if !principal_has_cap(principal, capability) {
            return Err(admin_err(
                StatusCode::FORBIDDEN,
                "forbidden",
                format!("missing capability `{capability}`"),
            )
            .into_response());
        }
    }
    let app_root = resolve_app_root(snap.workspace_root.as_path(), context.app_id.as_str());
    if !app_root.is_dir() {
        return Err(admin_err(StatusCode::NOT_FOUND, "not-found", "app not found").into_response());
    }
    Ok((resource, binding, app_root))
}

pub async fn api_config_record_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Query(query): Query<ProviderGetQuery>,
) -> impl IntoResponse {
    if !route_matches_context(&route, &query.context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    match load_provider_context(&snap, principal, &query.context) {
        Ok((resource, binding, app_root))
            if binding.provider_id == "config-record" && binding.method == "GET" =>
        {
            match get_config_record(&app_root, &binding) {
                Ok(record) => (
                    StatusCode::OK,
                    Json(json!({
                        "context": query.context,
                        "resourceKey": resource.registry_entry.resource_key,
                        "revision": record.revision,
                        "payload": record.data,
                        "binding": binding,
                    })),
                )
                    .into_response(),
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be config-record",
        )
        .into_response(),
        Err(response) => response,
    }
}

pub async fn api_config_record_put(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Json(body): Json<ConfigRecordPutBody>,
) -> impl IntoResponse {
    if !route_matches_context(&route, &body.context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    let actor = actor_from_principal(principal);
    let correlation_id = correlation_id(body.idempotency_key.as_deref());
    match load_provider_context(&snap, principal, &body.context) {
        Ok((resource, binding, app_root))
            if binding.provider_id == "config-record" && binding.method == "PUT" =>
        {
            if let Err(message) = validate_idempotency(&binding, body.idempotency_key.as_deref()) {
                return admin_err(StatusCode::BAD_REQUEST, "idempotency-invalid", message)
                    .into_response();
            }
            if let Err(message) = validate_revision(&binding, body.revision) {
                return admin_err(StatusCode::BAD_REQUEST, "revision-invalid", message)
                    .into_response();
            }
            if let Err(message) = validate_payload(&binding, &body.payload) {
                return admin_err(StatusCode::BAD_REQUEST, "payload-invalid", message)
                    .into_response();
            }
            match put_config_record(
                &app_root,
                &binding,
                body.revision.unwrap_or(0),
                body.payload,
                actor.as_str(),
                body.context.app_id.as_str(),
                resource.registry_entry.resource_id.as_str(),
                resource.registry_entry.module_id.as_str(),
                correlation_id.as_str(),
            ) {
                Ok(record) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "context": body.context,
                        "revision": record.revision,
                        "payload": record.data,
                        "applyPolicy": binding.apply_policy,
                        "danger": binding.danger,
                        "correlationId": correlation_id,
                    })),
                )
                    .into_response(),
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be config-record",
        )
        .into_response(),
        Err(response) => response,
    }
}

pub async fn api_asset_slot_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Query(query): Query<ProviderGetQuery>,
) -> impl IntoResponse {
    if !route_matches_context(&route, &query.context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    match load_provider_context(&snap, principal, &query.context) {
        Ok((_resource, binding, app_root))
            if binding.provider_id == "asset-slot"
                && matches!(binding.method.as_str(), "GET" | "LIST") =>
        {
            // Prefer the binding's own slot so each AssetSlot card stays isolated.
            // DataGrid may still call several list bindings and merge `slots`.
            match get_asset_slot(&app_root, &binding) {
                Ok(slot) => (
                    StatusCode::OK,
                    Json(json!({
                        "context": query.context,
                        "slots": [slot],
                    })),
                )
                    .into_response(),
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be asset-slot",
        )
        .into_response(),
        Err(response) => response,
    }
}

pub async fn api_asset_slot_replace(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Json(body): Json<AssetSlotReplaceBody>,
) -> impl IntoResponse {
    if !route_matches_context(&route, &body.context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    let bytes = match decode_bytes(body.content.as_deref(), body.content_hex.as_deref()) {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    let actor = actor_from_principal(principal);
    let correlation_id = correlation_id(body.idempotency_key.as_deref());
    match load_provider_context(&snap, principal, &body.context) {
        Ok((resource, binding, app_root))
            if binding.provider_id == "asset-slot"
                && matches!(binding.method.as_str(), "PUT" | "POST") =>
        {
            if let Err(message) = validate_idempotency(&binding, body.idempotency_key.as_deref()) {
                return admin_err(StatusCode::BAD_REQUEST, "idempotency-invalid", message)
                    .into_response();
            }
            if let Err(message) = validate_binary_payload(&binding) {
                return admin_err(StatusCode::BAD_REQUEST, "payload-invalid", message)
                    .into_response();
            }
            match replace_asset_slot(
                &app_root,
                &binding,
                body.filename.as_str(),
                bytes.as_slice(),
                actor.as_str(),
                body.context.app_id.as_str(),
                resource.registry_entry.resource_id.as_str(),
                resource.registry_entry.module_id.as_str(),
                correlation_id.as_str(),
            ) {
                Ok(slot) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "context": body.context,
                        "slot": slot,
                        "applyPolicy": binding.apply_policy,
                        "danger": binding.danger,
                    })),
                )
                    .into_response(),
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be asset-slot",
        )
        .into_response(),
        Err(response) => response,
    }
}

pub async fn api_asset_slot_apply_current(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Json(body): Json<AssetSlotApplyBody>,
) -> impl IntoResponse {
    if !route_matches_context(&route, &body.context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    let actor = actor_from_principal(principal);
    let correlation_id = correlation_id(body.idempotency_key.as_deref());
    match load_provider_context(&snap, principal, &body.context) {
        Ok((resource, binding, app_root)) if binding.provider_id == "asset-slot" => {
            match apply_asset_slot_current(
                &app_root,
                &binding,
                body.filename.as_str(),
                actor.as_str(),
                body.context.app_id.as_str(),
                resource.registry_entry.resource_id.as_str(),
                resource.registry_entry.module_id.as_str(),
                correlation_id.as_str(),
            ) {
                Ok(slot) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "context": body.context,
                        "slot": slot,
                        "applyPolicy": "restart-runtime",
                        "danger": binding.danger,
                    })),
                )
                    .into_response(),
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be asset-slot",
        )
        .into_response(),
        Err(response) => response,
    }
}

pub async fn api_asset_slot_delete_file(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Json(body): Json<AssetSlotDeleteBody>,
) -> impl IntoResponse {
    if !route_matches_context(&route, &body.context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    let actor = actor_from_principal(principal);
    let correlation_id = correlation_id(body.idempotency_key.as_deref());
    match load_provider_context(&snap, principal, &body.context) {
        Ok((resource, binding, app_root)) if binding.provider_id == "asset-slot" => {
            match delete_asset_slot_file(
                &app_root,
                &binding,
                body.filename.as_str(),
                actor.as_str(),
                body.context.app_id.as_str(),
                resource.registry_entry.resource_id.as_str(),
                resource.registry_entry.module_id.as_str(),
                correlation_id.as_str(),
            ) {
                Ok(slot) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "context": body.context,
                        "slot": slot,
                        "applyPolicy": binding.apply_policy,
                        "danger": binding.danger,
                    })),
                )
                    .into_response(),
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be asset-slot",
        )
        .into_response(),
        Err(response) => response,
    }
}

fn asset_slot_download_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("xls") => "application/vnd.ms-excel",
        Some("csv") => "text/csv; charset=utf-8",
        Some("json") | Some("geojson") => "application/json",
        _ => "application/octet-stream",
    }
}

fn asset_slot_attachment_disposition(file_name: &str) -> Result<HeaderValue, Response> {
    let safe = file_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    HeaderValue::from_str(&format!("attachment; filename=\"{safe}\"")).map_err(|_| {
        admin_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "header",
            "invalid Content-Disposition",
        )
        .into_response()
    })
}

fn respond_asset_slot_download(
    snap: &AdminRegistrySnapshot,
    principal: Option<&AuthPrincipal>,
    route: &ProviderRoute,
    context: &ProviderContext,
    filename: &str,
) -> Response {
    if !route_matches_context(route, context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    match load_provider_context(snap, principal, context) {
        Ok((_resource, binding, app_root)) if binding.provider_id == "asset-slot" => {
            match resolve_asset_slot_download_path(&app_root, &binding, filename) {
                Ok((name, path)) => match fs::read(&path) {
                    Ok(bytes) => {
                        let disposition = match asset_slot_attachment_disposition(name.as_str()) {
                            Ok(value) => value,
                            Err(response) => return response,
                        };
                        let mut response = Response::new(Body::from(bytes));
                        *response.status_mut() = StatusCode::OK;
                        response.headers_mut().insert(
                            header::CONTENT_TYPE,
                            HeaderValue::from_static(asset_slot_download_content_type(&path)),
                        );
                        response
                            .headers_mut()
                            .insert(header::CONTENT_DISPOSITION, disposition);
                        response
                    }
                    Err(error) => admin_err(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "io",
                        format!("failed to read asset slot file: {error}"),
                    )
                    .into_response(),
                },
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be asset-slot",
        )
        .into_response(),
        Err(response) => response,
    }
}

pub async fn api_asset_slot_download_file_post(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Json(body): Json<AssetSlotDownloadBody>,
) -> impl IntoResponse {
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    respond_asset_slot_download(
        &snap,
        principal,
        &route,
        &body.context,
        body.filename.as_str(),
    )
}

pub async fn api_asset_slot_download_file_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Query(query): Query<AssetSlotDownloadQuery>,
) -> impl IntoResponse {
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    respond_asset_slot_download(
        &snap,
        principal,
        &route,
        &query.context,
        query.filename.as_str(),
    )
}

pub async fn api_command_job_get(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Query(query): Query<ProviderGetQuery>,
) -> impl IntoResponse {
    if !route_matches_context(&route, &query.context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    match load_provider_context(&snap, principal, &query.context) {
        Ok((_resource, binding, app_root))
            if binding.provider_id == "command-job" && binding.method == "GET" =>
        {
            let Some(job_id) = query.job_id.as_deref() else {
                return admin_err(StatusCode::BAD_REQUEST, "validation", "jobId is required")
                    .into_response();
            };
            match get_command_job(&app_root, job_id) {
                Ok(job) => (StatusCode::OK, Json(json!({"job": job}))).into_response(),
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be command-job",
        )
        .into_response(),
        Err(response) => response,
    }
}

pub async fn api_command_job_run(
    State(state): State<SharedState>,
    principal: Option<Extension<AuthPrincipal>>,
    AxumPath(route): AxumPath<ProviderRoute>,
    Json(body): Json<CommandJobRunBody>,
) -> impl IntoResponse {
    if !route_matches_context(&route, &body.context) {
        return admin_err(
            StatusCode::BAD_REQUEST,
            "context-mismatch",
            "route and API context differ",
        )
        .into_response();
    }
    let bytes = match decode_bytes(body.content.as_deref(), body.content_hex.as_deref()) {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let snap = ensure_registry(&state);
    let principal = principal.as_ref().map(|value| &value.0);
    let actor = actor_from_principal(principal);
    let correlation_id = correlation_id(body.idempotency_key.as_deref());
    match load_provider_context(&snap, principal, &body.context) {
        Ok((resource, command_binding, app_root))
            if command_binding.provider_id == "command-job" && command_binding.method == "POST" =>
        {
            if let Err(message) =
                validate_idempotency(&command_binding, body.idempotency_key.as_deref())
            {
                return admin_err(StatusCode::BAD_REQUEST, "idempotency-invalid", message)
                    .into_response();
            }
            if let Err(message) = validate_binary_payload(&command_binding) {
                return admin_err(StatusCode::BAD_REQUEST, "payload-invalid", message)
                    .into_response();
            }
            let Some(asset_binding) =
                resource
                    .page_program
                    .provider_bindings
                    .iter()
                    .find(|binding| {
                        binding.binding_id == body.asset_binding_id
                            && binding.provider_id == "asset-slot"
                            && matches!(binding.method.as_str(), "PUT" | "POST")
                    })
            else {
                return admin_err(
                    StatusCode::FORBIDDEN,
                    "binding-mismatch",
                    "asset binding is not declared by this Admin scene",
                )
                .into_response();
            };
            if let Some(capability) = asset_binding
                .required_capabilities
                .iter()
                .find(|capability| !principal_has_cap(principal, capability))
            {
                return admin_err(
                    StatusCode::FORBIDDEN,
                    "forbidden",
                    format!("missing capability `{capability}`"),
                )
                .into_response();
            }
            match run_import_job(
                &app_root,
                &command_binding,
                asset_binding,
                body.context.app_id.as_str(),
                resource.registry_entry.resource_id.as_str(),
                resource.registry_entry.module_id.as_str(),
                body.filename.as_str(),
                bytes.as_slice(),
                actor.as_str(),
                correlation_id.as_str(),
            ) {
                Ok(job) => (
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "job": job,
                        "applyPolicy": command_binding.apply_policy,
                        "danger": command_binding.danger,
                    })),
                )
                    .into_response(),
                Err(error) => map_record_error(error).into_response(),
            }
        }
        Ok(_) => admin_err(
            StatusCode::BAD_REQUEST,
            "binding-mismatch",
            "provider must be command-job",
        )
        .into_response(),
        Err(response) => response,
    }
}

fn validate_idempotency(binding: &ProviderBinding, key: Option<&str>) -> Result<(), String> {
    let supplied = key.is_some_and(|value| !value.trim().is_empty());
    match binding.idempotency.as_str() {
        "required" if !supplied => Err("idempotencyKey is required by the binding".to_string()),
        "none" if supplied => Err("idempotencyKey is forbidden by the binding".to_string()),
        "required" | "optional" | "none" => Ok(()),
        value => Err(format!("unsupported idempotency policy `{value}`")),
    }
}

fn validate_revision(binding: &ProviderBinding, revision: Option<u64>) -> Result<(), String> {
    match binding.revision.as_str() {
        "required" if revision.is_none() => Err("revision is required by the binding".to_string()),
        "none" if revision.is_some() => Err("revision is forbidden by the binding".to_string()),
        "required" | "optional" | "none" => Ok(()),
        value => Err(format!("unsupported revision policy `{value}`")),
    }
}

fn validate_payload(binding: &ProviderBinding, payload: &Value) -> Result<(), String> {
    let valid = match binding.payload_type.name.as_str() {
        "json" | "json-object" | "object" => payload.is_object(),
        "array" | "json-array" => payload.is_array(),
        "string" => payload.is_string(),
        "boolean" | "bool" => payload.is_boolean(),
        "number" => payload.is_number(),
        "bytes" | "file" => payload.is_string(),
        _ => false,
    };
    if !valid {
        return Err(format!(
            "payload does not match declared type `{}`",
            binding.payload_type.name
        ));
    }
    if binding
        .validator
        .as_ref()
        .is_some_and(|validator| validator.reference.trim().is_empty())
    {
        return Err("validator reference is empty".to_string());
    }
    Ok(())
}

fn validate_binary_payload(binding: &ProviderBinding) -> Result<(), String> {
    if matches!(
        binding.payload_type.name.as_str(),
        "bytes" | "file" | "string"
    ) {
        Ok(())
    } else {
        Err(format!(
            "binary request does not match declared type `{}`",
            binding.payload_type.name
        ))
    }
}

fn correlation_id(idempotency_key: Option<&str>) -> String {
    format!(
        "admin-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0),
        idempotency_key
            .unwrap_or("none")
            .chars()
            .take(8)
            .collect::<String>()
    )
}

fn decode_bytes(
    content: Option<&str>,
    content_hex: Option<&str>,
) -> Result<Vec<u8>, ResponseTuple> {
    if let Some(hex) = content_hex.map(str::trim).filter(|value| !value.is_empty()) {
        if hex.len() % 2 != 0 {
            return Err(admin_err(
                StatusCode::BAD_REQUEST,
                "validation",
                "contentHex length must be even",
            ));
        }
        return (0..hex.len())
            .step_by(2)
            .map(|index| {
                u8::from_str_radix(&hex[index..index + 2], 16).map_err(|error| {
                    admin_err(StatusCode::BAD_REQUEST, "validation", error.to_string())
                })
            })
            .collect();
    }
    content
        .map(|value| value.as_bytes().to_vec())
        .ok_or_else(|| {
            admin_err(
                StatusCode::BAD_REQUEST,
                "validation",
                "content or contentHex is required",
            )
        })
}

fn map_record_error(error: AdminRecordError) -> ResponseTuple {
    match error {
        AdminRecordError::Conflict { current_revision } => (
            StatusCode::CONFLICT,
            Json(AdminErrorBody {
                kind: "conflict",
                message: "stale revision".to_string(),
                current_revision: Some(current_revision),
            }),
        ),
        AdminRecordError::Validation(message) => {
            admin_err(StatusCode::BAD_REQUEST, "validation", message)
        }
        AdminRecordError::NotFound(message) => {
            admin_err(StatusCode::NOT_FOUND, "not-found", message)
        }
        AdminRecordError::Parse(message) | AdminRecordError::Io(message) => {
            admin_err(StatusCode::INTERNAL_SERVER_ERROR, "internal", message)
        }
    }
}
