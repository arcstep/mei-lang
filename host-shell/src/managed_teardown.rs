//! Shared teardown for managed children after HTTP serve returns.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app_runtime_supervisor::SharedAppRuntime;
use crate::managed_martin::ManagedMartin;
use crate::managed_plug::ManagedPlugDsPool;
use crate::runtime_actor::RuntimeActorHandle;

const TEARDOWN_TIMEOUT: Duration = Duration::from_secs(5);

pub async fn teardown_managed_children(
    runtime_actor: Option<RuntimeActorHandle>,
    managed_plug: Option<Arc<Mutex<Option<ManagedPlugDsPool>>>>,
    managed_martin: &Arc<Mutex<Option<ManagedMartin>>>,
    app_runtime: &SharedAppRuntime,
) {
    match tokio::time::timeout(
        TEARDOWN_TIMEOUT,
        teardown_managed_children_inner(runtime_actor, managed_plug, managed_martin, app_runtime),
    )
    .await
    {
        Ok(()) => {}
        Err(_) => {
            tracing::error!(
                timeout_secs = TEARDOWN_TIMEOUT.as_secs(),
                "managed children teardown timed out"
            );
        }
    }
}

async fn teardown_managed_children_inner(
    runtime_actor: Option<RuntimeActorHandle>,
    managed_plug: Option<Arc<Mutex<Option<ManagedPlugDsPool>>>>,
    managed_martin: &Arc<Mutex<Option<ManagedMartin>>>,
    app_runtime: &SharedAppRuntime,
) {
    if let Some(actor) = runtime_actor {
        actor.shutdown().await;
    }
    if let Some(slot) = managed_plug {
        if let Some(mut pool) = slot.lock().ok().and_then(|mut guard| guard.take()) {
            if let Err(error) = pool.shutdown().await {
                tracing::warn!(detail = %error, "managed plug-ds pool shutdown failed");
            }
        }
    }
    crate::managed_martin::shutdown_managed_martin_slot(managed_martin).await;
    {
        let mut supervisor = app_runtime.lock().await;
        if let Err(error) = supervisor.shutdown_all().await {
            tracing::warn!(detail = %error, "app-runtime supervisor shutdown failed");
        }
    }
}
