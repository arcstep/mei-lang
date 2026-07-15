//! Host runtime command router with one serial lane per app.
//!
//! Commands for the same app preserve arrival order (including Start → Stop), while
//! different app lanes run independently. A global semaphore caps simultaneous slow
//! starts so a restart storm cannot exhaust the Host control plane.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::json;
use tokio::sync::{mpsc, oneshot, Semaphore};

use crate::app_launch_api::{start_app_with_launch, stop_app_runtime, StartStopError};
use crate::state::{HostEvent, HostHttpState};

pub enum RuntimeCommand {
    Start {
        app_id: String,
        config: Option<String>,
        mode: Option<String>,
        follow_git: bool,
        reply: oneshot::Sender<Result<serde_json::Value, StartStopError>>,
    },
    Stop {
        app_id: String,
        reply: oneshot::Sender<Result<serde_json::Value, StartStopError>>,
    },
    StartFinished {
        app_id: String,
    },
    Shutdown,
}

enum LaneCommand {
    Start {
        app_id: String,
        config: Option<String>,
        mode: Option<String>,
        follow_git: bool,
        reply: oneshot::Sender<Result<serde_json::Value, StartStopError>>,
    },
    Stop {
        app_id: String,
        reply: oneshot::Sender<Result<serde_json::Value, StartStopError>>,
    },
}

#[derive(Clone)]
pub struct RuntimeActorHandle {
    tx: mpsc::Sender<RuntimeCommand>,
}

impl RuntimeActorHandle {
    pub fn spawn(http: HostHttpState) -> Self {
        let (tx, mut rx) = mpsc::channel::<RuntimeCommand>(64);
        let router_tx = tx.clone();
        let start_limit = Arc::new(Semaphore::new(start_concurrency_limit()));
        tokio::spawn(async move {
            let mut lanes = BTreeMap::<String, mpsc::Sender<LaneCommand>>::new();
            let mut starts_inflight = BTreeSet::<String>::new();
            while let Some(cmd) = rx.recv().await {
                match cmd {
                    RuntimeCommand::Shutdown => break,
                    RuntimeCommand::Start {
                        app_id,
                        config,
                        mode,
                        follow_git,
                        reply,
                    } => {
                        if !starts_inflight.insert(app_id.clone()) {
                            emit_start_rejected(&http, app_id.as_str());
                            let _ = reply.send(Err(StartStopError::Conflict(format!(
                                "app-start-in-flight: app `{app_id}` is already queued"
                            ))));
                            continue;
                        }
                        emit_start_queued(&http, app_id.as_str());
                        let lane = lane_for(
                            &mut lanes,
                            app_id.as_str(),
                            http.clone(),
                            router_tx.clone(),
                            start_limit.clone(),
                        );
                        if let Err(error) = lane
                            .send(LaneCommand::Start {
                                app_id: app_id.clone(),
                                config,
                                mode,
                                follow_git,
                                reply,
                            })
                            .await
                        {
                            starts_inflight.remove(app_id.as_str());
                            if let LaneCommand::Start { reply, .. } = error.0 {
                                let _ = reply.send(Err(StartStopError::Unavailable(
                                    "runtime app lane unavailable".into(),
                                )));
                            }
                            lanes.remove(app_id.as_str());
                        }
                    }
                    RuntimeCommand::Stop { app_id, reply } => {
                        let lane = lane_for(
                            &mut lanes,
                            app_id.as_str(),
                            http.clone(),
                            router_tx.clone(),
                            start_limit.clone(),
                        );
                        if let Err(error) = lane
                            .send(LaneCommand::Stop {
                                app_id: app_id.clone(),
                                reply,
                            })
                            .await
                        {
                            if let LaneCommand::Stop { reply, .. } = error.0 {
                                let _ = reply.send(Err(StartStopError::Unavailable(
                                    "runtime app lane unavailable".into(),
                                )));
                            }
                            lanes.remove(app_id.as_str());
                        }
                    }
                    RuntimeCommand::StartFinished { app_id } => {
                        starts_inflight.remove(app_id.as_str());
                    }
                }
            }
        });
        Self { tx }
    }

    pub async fn start(
        &self,
        app_id: &str,
        config: Option<&str>,
        mode: Option<&str>,
        follow_git: bool,
    ) -> Result<serde_json::Value, StartStopError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(RuntimeCommand::Start {
                app_id: app_id.to_string(),
                config: config.map(str::to_string),
                mode: mode.map(str::to_string),
                follow_git,
                reply,
            })
            .await
            .map_err(|_| StartStopError::Unavailable("runtime actor unavailable".into()))?;
        rx.await
            .map_err(|_| StartStopError::Unavailable("runtime actor dropped reply".into()))?
    }

    pub async fn stop(&self, app_id: &str) -> Result<serde_json::Value, StartStopError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(RuntimeCommand::Stop {
                app_id: app_id.to_string(),
                reply,
            })
            .await
            .map_err(|_| StartStopError::Unavailable("runtime actor unavailable".into()))?;
        rx.await
            .map_err(|_| StartStopError::Unavailable("runtime actor dropped reply".into()))?
    }

    pub async fn shutdown(&self) {
        let _ = self.tx.send(RuntimeCommand::Shutdown).await;
    }
}

fn lane_for(
    lanes: &mut BTreeMap<String, mpsc::Sender<LaneCommand>>,
    app_id: &str,
    http: HostHttpState,
    router_tx: mpsc::Sender<RuntimeCommand>,
    start_limit: Arc<Semaphore>,
) -> mpsc::Sender<LaneCommand> {
    if let Some(lane) = lanes.get(app_id) {
        return lane.clone();
    }
    let (tx, mut rx) = mpsc::channel::<LaneCommand>(16);
    tokio::spawn(async move {
        while let Some(command) = rx.recv().await {
            match command {
                LaneCommand::Start {
                    app_id,
                    config,
                    mode,
                    follow_git,
                    reply,
                } => {
                    let result = match start_limit.clone().acquire_owned().await {
                        Ok(_permit) => {
                            start_app_with_launch(
                                &http,
                                app_id.as_str(),
                                config.as_deref(),
                                mode.as_deref(),
                                follow_git,
                            )
                            .await
                        }
                        Err(_) => Err(StartStopError::Unavailable(
                            "runtime start limiter unavailable".into(),
                        )),
                    };
                    if let Err(StartStopError::Conflict(message)) = &result {
                        if message.starts_with("app-start-in-flight") {
                            emit_start_rejected(&http, app_id.as_str());
                        }
                    }
                    let _ = reply.send(result);
                    let _ = router_tx
                        .send(RuntimeCommand::StartFinished { app_id })
                        .await;
                }
                LaneCommand::Stop { app_id, reply } => {
                    let result = stop_app_runtime(&http, app_id.as_str()).await;
                    let _ = reply.send(result);
                }
            }
        }
    });
    lanes.insert(app_id.to_string(), tx.clone());
    tx
}

fn start_concurrency_limit() -> usize {
    std::env::var("MEI_HOST_RUNTIME_START_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(4)
}

fn emit_start_queued(http: &HostHttpState, app_id: &str) {
    let guard = http.shell.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(
        "app-start-queued",
        json!({
            "appId": app_id,
            "phase": "queued",
        }),
    ));
}

fn emit_start_rejected(http: &HostHttpState, app_id: &str) {
    let message = format!("app-start-in-flight: app `{app_id}` is already queued");
    let guard = http.shell.read().expect("state lock");
    let _ = guard.events.send(HostEvent::new(
        "app-start-rejected",
        json!({
            "appId": app_id,
            "kind": "app-start-in-flight",
            "message": message,
            "inflightRejected": true,
        }),
    ));
}

#[cfg(test)]
mod tests {
    use super::start_concurrency_limit;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn start_limit_defaults_and_rejects_zero() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        std::env::remove_var("MEI_HOST_RUNTIME_START_CONCURRENCY");
        assert_eq!(start_concurrency_limit(), 4);
        std::env::set_var("MEI_HOST_RUNTIME_START_CONCURRENCY", "0");
        assert_eq!(start_concurrency_limit(), 4);
        std::env::set_var("MEI_HOST_RUNTIME_START_CONCURRENCY", "2");
        assert_eq!(start_concurrency_limit(), 2);
        std::env::remove_var("MEI_HOST_RUNTIME_START_CONCURRENCY");
    }
}
