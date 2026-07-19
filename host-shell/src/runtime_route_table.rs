//! Atomic read snapshot for app-runtime proxy identity.
//!
//! Writers publish a new `Arc` on cutover / start / stop. Readers never take
//! the supervisor mutex, so spawn of app A cannot blind proxy for app B.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use serde::Serialize;

use crate::app_runtime_proxy::RuntimeProxyIdentity;
use mei_host_auth::AuthPrincipal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RuntimeRoutePhase {
    Starting,
    Running,
    Draining,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRouteEntry {
    pub app_id: String,
    pub instance_id: String,
    pub endpoint: String,
    pub token: String,
    pub generation: String,
    pub spec_digest: String,
    pub phase: RuntimeRoutePhase,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeRouteTable {
    /// app_id → entry
    entries: BTreeMap<String, RuntimeRouteEntry>,
    pub version: u64,
}

impl RuntimeRouteTable {
    pub fn get(&self, app_id: &str) -> Option<&RuntimeRouteEntry> {
        self.entries.get(app_id)
    }

    pub fn identity_for(
        &self,
        app_id: &str,
        principal: Option<AuthPrincipal>,
    ) -> Option<RuntimeProxyIdentity> {
        let entry = self.entries.get(app_id)?;
        if entry.phase != RuntimeRoutePhase::Running {
            return None;
        }
        if entry.endpoint.trim().is_empty() || entry.endpoint.starts_with("pending://") {
            return None;
        }
        Some(RuntimeProxyIdentity {
            endpoint: entry.endpoint.clone(),
            token: entry.token.clone(),
            instance_id: entry.instance_id.clone(),
            app_id: entry.app_id.clone(),
            generation: entry.generation.clone(),
            spec_digest: entry.spec_digest.clone(),
            principal,
        })
    }
}

/// Lock-freeish publish: readers clone Arc; writers replace Arc under write lock.
#[derive(Clone, Default)]
pub struct SharedRuntimeRouteTable {
    inner: Arc<RwLock<Arc<RuntimeRouteTable>>>,
}

impl SharedRuntimeRouteTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(&self) -> Arc<RuntimeRouteTable> {
        self.inner
            .read()
            .map(|guard| Arc::clone(&*guard))
            .unwrap_or_default()
    }

    pub fn publish_entry(&self, entry: RuntimeRouteEntry) {
        let Ok(mut guard) = self.inner.write() else {
            return;
        };
        let mut next = (**guard).clone();
        next.entries.insert(entry.app_id.clone(), entry);
        next.version = next.version.saturating_add(1);
        *guard = Arc::new(next);
    }

    pub fn set_phase(&self, app_id: &str, phase: RuntimeRoutePhase) {
        let Ok(mut guard) = self.inner.write() else {
            return;
        };
        let mut next = (**guard).clone();
        if let Some(entry) = next.entries.get_mut(app_id) {
            entry.phase = phase;
            next.version = next.version.saturating_add(1);
            *guard = Arc::new(next);
        }
    }

    pub fn remove_app(&self, app_id: &str) {
        let Ok(mut guard) = self.inner.write() else {
            return;
        };
        let mut next = (**guard).clone();
        if next.entries.remove(app_id).is_some() {
            next.version = next.version.saturating_add(1);
            *guard = Arc::new(next);
        }
    }

    /// Rebuild app_id → entry from instance maps + launch routes.
    pub fn rebuild_from_shell(
        &self,
        routes: &BTreeMap<String, mei_host_core::RouteBinding>,
        endpoints: &BTreeMap<String, String>,
        tokens: &BTreeMap<String, String>,
        generations: &BTreeMap<String, String>,
        digests: &BTreeMap<String, String>,
    ) {
        let mut next = RuntimeRouteTable::default();
        for (app_id, binding) in routes {
            let Some(instance_id) = binding.active.as_ref() else {
                continue;
            };
            let Some(endpoint) = endpoints.get(instance_id) else {
                continue;
            };
            let token = tokens.get(instance_id).cloned().unwrap_or_default();
            next.entries.insert(
                app_id.clone(),
                RuntimeRouteEntry {
                    app_id: app_id.clone(),
                    instance_id: instance_id.clone(),
                    endpoint: endpoint.clone(),
                    token,
                    generation: generations.get(instance_id).cloned().unwrap_or_default(),
                    spec_digest: digests.get(instance_id).cloned().unwrap_or_default(),
                    phase: RuntimeRoutePhase::Running,
                },
            );
        }
        next.version = self
            .inner
            .read()
            .map(|g| g.version.saturating_add(1))
            .unwrap_or(1);
        if let Ok(mut guard) = self.inner.write() {
            *guard = Arc::new(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_entry_resolves_identity_starting_does_not() {
        let table = SharedRuntimeRouteTable::new();
        table.publish_entry(RuntimeRouteEntry {
            app_id: "zhifa".into(),
            instance_id: "zhifa:1".into(),
            endpoint: "http://127.0.0.1:19001".into(),
            token: "tok".into(),
            generation: "g1".into(),
            spec_digest: "d1".into(),
            phase: RuntimeRoutePhase::Running,
        });
        assert!(table.load().identity_for("zhifa", None).is_some());

        table.publish_entry(RuntimeRouteEntry {
            app_id: "mini-data".into(),
            instance_id: "mini-data:starting".into(),
            endpoint: "pending://starting".into(),
            token: String::new(),
            generation: String::new(),
            spec_digest: String::new(),
            phase: RuntimeRoutePhase::Starting,
        });
        assert!(table.load().identity_for("mini-data", None).is_none());
        // Other app remains visible while another is starting.
        assert!(table.load().identity_for("zhifa", None).is_some());
    }

    #[test]
    fn draining_clears_proxy_identity() {
        let table = SharedRuntimeRouteTable::new();
        table.publish_entry(RuntimeRouteEntry {
            app_id: "zhifa".into(),
            instance_id: "zhifa:1".into(),
            endpoint: "http://127.0.0.1:19001".into(),
            token: "tok".into(),
            generation: "g1".into(),
            spec_digest: "d1".into(),
            phase: RuntimeRoutePhase::Running,
        });
        table.set_phase("zhifa", RuntimeRoutePhase::Draining);
        assert!(table.load().identity_for("zhifa", None).is_none());
    }
}
