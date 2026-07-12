use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
};
use tokio::sync::broadcast;

use crate::state::SharedState;

pub async fn api_host_events(State(state): State<SharedState>) -> Response {
    let mut receiver = {
        let guard = state.read().expect("state lock");
        guard.events.subscribe()
    };
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(message) => {
                    let data = serde_json::to_string(&message).unwrap_or_else(|_| "{}".to_string());
                    yield Ok::<Event, Infallible>(
                        Event::default().event(message.event_type).data(data)
                    );
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
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
