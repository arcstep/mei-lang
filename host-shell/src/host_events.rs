use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use axum::{
    extract::{Query, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::state::{HostEventTelemetry, SharedState};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostEventQuery {
    client_id: Option<String>,
    leader: Option<String>,
}

struct HostEventStreamLifecycle {
    telemetry: Arc<HostEventTelemetry>,
    opened_at: Instant,
    client_id: String,
    leader_kind: String,
}

impl Drop for HostEventStreamLifecycle {
    fn drop(&mut self) {
        let snapshot = self.telemetry.stream_closed();
        tracing::debug!(
            target: "mei.host.events",
            client_id = %self.client_id,
            leader_kind = %self.leader_kind,
            duration_ms = self.opened_at.elapsed().as_millis() as u64,
            active = snapshot.active,
            opened_total = snapshot.opened_total,
            closed_total = snapshot.closed_total,
            "host event stream closed"
        );
    }
}

fn bounded_label(value: Option<String>, fallback: &str) -> String {
    let value = value.unwrap_or_else(|| fallback.to_string());
    value.trim().chars().take(96).collect()
}

pub async fn api_host_events(
    State(state): State<SharedState>,
    Query(query): Query<HostEventQuery>,
) -> Response {
    let (mut receiver, telemetry) = {
        let guard = state.read().expect("state lock");
        (guard.events.subscribe(), guard.event_telemetry.clone())
    };
    let client_id = bounded_label(query.client_id, "unknown");
    let leader_kind = bounded_label(query.leader, "legacy");
    let snapshot = telemetry.stream_opened();
    tracing::debug!(
        target: "mei.host.events",
        client_id = %client_id,
        leader_kind = %leader_kind,
        active = snapshot.active,
        opened_total = snapshot.opened_total,
        "host event stream opened"
    );
    let lifecycle = HostEventStreamLifecycle {
        telemetry,
        opened_at: Instant::now(),
        client_id,
        leader_kind,
    };
    let stream = async_stream::stream! {
        let lifecycle = lifecycle;
        loop {
            match receiver.recv().await {
                Ok(message) => {
                    let data = serde_json::to_string(&message).unwrap_or_else(|_| "{}".to_string());
                    yield Ok::<Event, Infallible>(
                        Event::default().event(message.event_type).data(data)
                    );
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    let snapshot = lifecycle.telemetry.record_lagged(skipped);
                    tracing::warn!(
                        target: "mei.host.events",
                        client_id = %lifecycle.client_id,
                        leader_kind = %lifecycle.leader_kind,
                        skipped,
                        lagged_messages = snapshot.lagged_messages,
                        "host event stream lagged"
                    );
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}
