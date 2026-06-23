use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::Query,
    http::{header::CONTENT_LENGTH, Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::http::startup_run;

const DEFAULT_CAPACITY: usize = 10_000;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RequestTraceRecord {
    pub seq: u64,
    #[serde(rename = "recordedAtMs")]
    pub recorded_at_ms: u128,
    #[serde(rename = "requestId")]
    pub request_id: String,
    #[serde(rename = "runId", skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub method: String,
    pub uri: String,
    #[serde(rename = "routeKind")]
    pub route_kind: String,
    #[serde(rename = "appId")]
    pub app_id: String,
    pub status: u16,
    #[serde(rename = "latencyMs")]
    pub latency_ms: u128,
    #[serde(rename = "requestBytes")]
    pub request_bytes: u64,
    #[serde(rename = "responseBytes")]
    pub response_bytes: u64,
}

#[derive(Debug, Default)]
struct RequestTraceStore {
    records: VecDeque<RequestTraceRecord>,
    next_seq: u64,
    total_recorded: u64,
    capacity: usize,
}

impl RequestTraceStore {
    fn capacity() -> usize {
        static CAPACITY: OnceLock<usize> = OnceLock::new();
        *CAPACITY.get_or_init(|| {
            std::env::var("MEI_REQUEST_TRACE_CAPACITY")
                .ok()
                .and_then(|raw| raw.parse().ok())
                .filter(|value| *value >= 100)
                .unwrap_or(DEFAULT_CAPACITY)
        })
    }

    fn push(&mut self, mut record: RequestTraceRecord) -> RequestTraceRecord {
        self.next_seq = self.next_seq.saturating_add(1);
        record.seq = self.next_seq;
        if self.records.len() >= self.capacity {
            self.records.pop_front();
        }
        self.records.push_back(record.clone());
        self.total_recorded = self.total_recorded.saturating_add(1);
        record
    }
}

fn store() -> &'static Mutex<RequestTraceStore> {
    static STORE: OnceLock<Mutex<RequestTraceStore>> = OnceLock::new();
    STORE.get_or_init(|| {
        Mutex::new(RequestTraceStore {
            capacity: RequestTraceStore::capacity(),
            ..RequestTraceStore::default()
        })
    })
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(crate) fn classify_route(method: &Method, uri: &Uri) -> (String, String) {
    let path = uri.path();
    let app_tail = |prefix: &str| {
        path.strip_prefix(prefix)
            .unwrap_or_default()
            .trim_start_matches('/')
            .to_string()
    };
    if *method == Method::GET && path.starts_with("/apps/presentation/") {
        return (
            "presentation_page".to_string(),
            app_tail("/apps/presentation/"),
        );
    }
    if *method == Method::GET && path.starts_with("/apps/build/") {
        return ("build_page".to_string(), app_tail("/apps/build/"));
    }
    if *method == Method::GET && path.starts_with("/apps/app/") {
        return ("app_page".to_string(), app_tail("/apps/app/"));
    }
    if *method == Method::GET && path.starts_with("/apps/config/") {
        return ("config_page".to_string(), app_tail("/apps/config/"));
    }
    if *method == Method::GET && path.starts_with("/apps/upload/") {
        return ("upload_page".to_string(), app_tail("/apps/upload/"));
    }
    if *method == Method::GET && path.starts_with("/apps/manage/") {
        return ("manage_page_legacy".to_string(), app_tail("/apps/manage/"));
    }
    if *method == Method::GET && path.starts_with("/apps/access/") {
        return ("access_page_legacy".to_string(), app_tail("/apps/access/"));
    }
    if *method == Method::POST && path.starts_with("/api/datasets/query/") {
        return (
            "dataset_query".to_string(),
            app_tail("/api/datasets/query/"),
        );
    }
    if *method == Method::POST && path.starts_with("/api/datasets/metrics/") {
        return (
            "metric_query".to_string(),
            app_tail("/api/datasets/metrics/"),
        );
    }
    if *method == Method::POST && path.starts_with("/api/datasets/recompute/") {
        return (
            "dataset_recompute".to_string(),
            app_tail("/api/datasets/recompute/"),
        );
    }
    if path.starts_with("/api/") {
        return ("api".to_string(), path.trim_start_matches('/').to_string());
    }
    ("http_request".to_string(), String::new())
}

pub(crate) fn should_persist_trace(method: &Method, uri: &Uri) -> bool {
    let path = uri.path();
    if path.starts_with("/api/") {
        return true;
    }
    if path.starts_with("/apps/") {
        return true;
    }
    if *method != Method::GET {
        return false;
    }
    !path.starts_with("/app-assets/")
        && !path.starts_with("/workspace-app-assets/")
        && !path.starts_with("/workspace-components/")
        && !path.starts_with("/gis/")
        && path != "/favicon.ico"
}

pub(crate) fn request_content_length(headers: &axum::http::HeaderMap) -> u64 {
    headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

pub(crate) fn record_request(
    request_id: &str,
    method: &Method,
    uri: &Uri,
    route_kind: &str,
    app_id: &str,
    status: StatusCode,
    latency_ms: u128,
    request_bytes: u64,
    response_bytes: u64,
) {
    if !should_persist_trace(method, uri) {
        return;
    }
    let record = RequestTraceRecord {
        seq: 0,
        recorded_at_ms: now_ms(),
        request_id: request_id.to_string(),
        run_id: startup_run::current_run_id(),
        method: method.to_string(),
        uri: uri.to_string(),
        route_kind: route_kind.to_string(),
        app_id: app_id.to_string(),
        status: status.as_u16(),
        latency_ms,
        request_bytes,
        response_bytes,
    };
    if let Ok(mut guard) = store().lock() {
        let persisted = guard.push(record);
        drop(guard);
        startup_run::write_request_trace_record(&persisted);
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct RequestTraceQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    #[serde(rename = "minSeq")]
    pub min_seq: Option<u64>,
    #[serde(rename = "routeKind")]
    pub route_kind: Option<String>,
    #[serde(rename = "appId")]
    pub app_id: Option<String>,
    #[serde(rename = "runId")]
    pub run_id: Option<String>,
    #[serde(rename = "minLatencyMs")]
    pub min_latency_ms: Option<u128>,
    pub summary: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RequestTraceListResponse {
    pub capacity: usize,
    pub count: usize,
    #[serde(rename = "totalRecorded")]
    pub total_recorded: u64,
    #[serde(rename = "firstSeq")]
    pub first_seq: Option<u64>,
    #[serde(rename = "lastSeq")]
    pub last_seq: Option<u64>,
    pub records: Vec<RequestTraceRecord>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RouteKindSummary {
    count: usize,
    #[serde(rename = "latencyMsTotal")]
    latency_ms_total: u128,
    #[serde(rename = "latencyMsMax")]
    latency_ms_max: u128,
    #[serde(rename = "responseBytesTotal")]
    response_bytes_total: u64,
    #[serde(rename = "responseBytesMax")]
    response_bytes_max: u64,
}

#[derive(Debug, Serialize)]
pub(crate) struct RequestTraceSummaryResponse {
    pub capacity: usize,
    pub count: usize,
    #[serde(rename = "totalRecorded")]
    pub total_recorded: u64,
    #[serde(rename = "firstSeq")]
    pub first_seq: Option<u64>,
    #[serde(rename = "lastSeq")]
    pub last_seq: Option<u64>,
    #[serde(rename = "latencyMsTotal")]
    pub latency_ms_total: u128,
    #[serde(rename = "latencyMsMax")]
    pub latency_ms_max: u128,
    #[serde(rename = "responseBytesTotal")]
    pub response_bytes_total: u64,
    #[serde(rename = "responseBytesMax")]
    pub response_bytes_max: u64,
    #[serde(rename = "byRouteKind")]
    pub by_route_kind: HashMap<String, RouteKindSummary>,
}

fn filter_records(
    records: &[RequestTraceRecord],
    query: &RequestTraceQuery,
) -> Vec<RequestTraceRecord> {
    records
        .iter()
        .rev()
        .filter(|record| {
            if let Some(min_seq) = query.min_seq {
                if record.seq < min_seq {
                    return false;
                }
            }
            if let Some(route_kind) = query.route_kind.as_deref() {
                if !record.route_kind.eq_ignore_ascii_case(route_kind) {
                    return false;
                }
            }
            if let Some(app_id) = query.app_id.as_deref() {
                if !record.app_id.eq_ignore_ascii_case(app_id) {
                    return false;
                }
            }
            if let Some(run_id) = query.run_id.as_deref() {
                if record.run_id.as_deref() != Some(run_id) {
                    return false;
                }
            }
            if let Some(min_latency_ms) = query.min_latency_ms {
                if record.latency_ms < min_latency_ms {
                    return false;
                }
            }
            true
        })
        .cloned()
        .collect()
}

fn build_summary(
    records: Vec<RequestTraceRecord>,
    capacity: usize,
    total_recorded: u64,
) -> RequestTraceSummaryResponse {
    let count = records.len();
    let first_seq = records.iter().map(|record| record.seq).min();
    let last_seq = records.iter().map(|record| record.seq).max();
    let mut latency_ms_total = 0_u128;
    let mut latency_ms_max = 0_u128;
    let mut response_bytes_total = 0_u64;
    let mut response_bytes_max = 0_u64;
    let mut by_route_kind: HashMap<String, RouteKindSummary> = HashMap::new();

    for record in &records {
        latency_ms_total = latency_ms_total.saturating_add(record.latency_ms);
        latency_ms_max = latency_ms_max.max(record.latency_ms);
        response_bytes_total = response_bytes_total.saturating_add(record.response_bytes);
        response_bytes_max = response_bytes_max.max(record.response_bytes);
        let entry = by_route_kind
            .entry(record.route_kind.clone())
            .or_insert_with(|| RouteKindSummary {
                count: 0,
                latency_ms_total: 0,
                latency_ms_max: 0,
                response_bytes_total: 0,
                response_bytes_max: 0,
            });
        entry.count += 1;
        entry.latency_ms_total = entry.latency_ms_total.saturating_add(record.latency_ms);
        entry.latency_ms_max = entry.latency_ms_max.max(record.latency_ms);
        entry.response_bytes_total = entry
            .response_bytes_total
            .saturating_add(record.response_bytes);
        entry.response_bytes_max = entry.response_bytes_max.max(record.response_bytes);
    }

    RequestTraceSummaryResponse {
        capacity,
        count,
        total_recorded,
        first_seq,
        last_seq,
        latency_ms_total,
        latency_ms_max,
        response_bytes_total,
        response_bytes_max,
        by_route_kind,
    }
}

pub async fn api_request_trace(Query(query): Query<RequestTraceQuery>) -> Response {
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let offset = query.offset.unwrap_or(0);
    let summary_mode = query
        .summary
        .as_deref()
        .is_some_and(|value| matches!(value, "1" | "true" | "yes"));

    let snapshot = store()
        .lock()
        .map(|guard| {
            (
                guard.capacity,
                guard.total_recorded,
                guard.records.iter().cloned().collect::<Vec<_>>(),
            )
        })
        .unwrap_or((DEFAULT_CAPACITY, 0, Vec::new()));

    let (capacity, total_recorded, all_records) = snapshot;
    let filtered = filter_records(&all_records, &query);

    if summary_mode {
        let summary = build_summary(filtered, capacity, total_recorded);
        if let (Some(requested_run_id), Some(active_run_id)) =
            (query.run_id.as_deref(), startup_run::current_run_id())
        {
            if requested_run_id == active_run_id {
                startup_run::write_request_trace_summary(&summary);
            }
        }
        return (StatusCode::OK, Json(summary)).into_response();
    }

    let records = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let count = records.len();
    let first_seq = records.iter().map(|record| record.seq).min();
    let last_seq = records.iter().map(|record| record.seq).max();

    let response = RequestTraceListResponse {
        capacity,
        count,
        total_recorded,
        first_seq,
        last_seq,
        records,
    };
    (StatusCode::OK, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use axum::http::Uri;

    use super::*;

    #[test]
    fn classify_dataset_query_route() {
        let uri: Uri = "/api/datasets/query/zhifa".parse().expect("uri");
        let (route_kind, app_id) = classify_route(&Method::POST, &uri);
        assert_eq!(route_kind, "dataset_query");
        assert_eq!(app_id, "zhifa");
    }

    #[test]
    fn ring_buffer_evicts_oldest_when_full() {
        let mut store = RequestTraceStore {
            records: VecDeque::new(),
            next_seq: 0,
            total_recorded: 0,
            capacity: 3,
        };
        for index in 0..5 {
            store.push(RequestTraceRecord {
                seq: 0,
                recorded_at_ms: index,
                request_id: format!("req-{index}"),
                run_id: None,
                method: "GET".into(),
                uri: "/api/host/ready".into(),
                route_kind: "api".into(),
                app_id: String::new(),
                status: 200,
                latency_ms: index,
                request_bytes: 0,
                response_bytes: index as u64,
            });
        }
        assert_eq!(store.records.len(), 3);
        assert_eq!(
            store.records.front().map(|r| r.request_id.as_str()),
            Some("req-2")
        );
        assert_eq!(
            store.records.back().map(|r| r.request_id.as_str()),
            Some("req-4")
        );
        assert_eq!(store.total_recorded, 5);
    }
}
