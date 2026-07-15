//! Per-app start/stop in-flight tracking (UI double-click / concurrent start).

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StartPhase {
    Prebuilding,
    Spawning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInflightEntry {
    pub app_id: String,
    pub phase: StartPhase,
    pub started_at_ms: u64,
}

#[derive(Default)]
pub struct AppStartInflight {
    map: Mutex<BTreeMap<String, StartInflightEntry>>,
}

impl AppStartInflight {
    pub fn try_begin(&self, app_id: &str, phase: StartPhase) -> Result<(), StartInflightEntry> {
        let mut guard = self.map.lock().expect("start inflight lock");
        if let Some(existing) = guard.get(app_id) {
            return Err(existing.clone());
        }
        guard.insert(
            app_id.to_string(),
            StartInflightEntry {
                app_id: app_id.to_string(),
                phase,
                started_at_ms: crate::state::current_time_ms(),
            },
        );
        Ok(())
    }

    pub fn set_phase(&self, app_id: &str, phase: StartPhase) {
        let mut guard = self.map.lock().expect("start inflight lock");
        if let Some(entry) = guard.get_mut(app_id) {
            entry.phase = phase;
        }
    }

    pub fn finish(&self, app_id: &str) {
        let mut guard = self.map.lock().expect("start inflight lock");
        guard.remove(app_id);
    }

    pub fn get(&self, app_id: &str) -> Option<StartInflightEntry> {
        self.map
            .lock()
            .ok()
            .and_then(|guard| guard.get(app_id).cloned())
    }
}

/// RAII guard that clears inflight on drop (including panic / early return).
pub struct StartInflightGuard<'a> {
    store: &'a AppStartInflight,
    app_id: String,
}

impl<'a> StartInflightGuard<'a> {
    pub fn try_acquire(
        store: &'a AppStartInflight,
        app_id: &str,
        phase: StartPhase,
    ) -> Result<Self, StartInflightEntry> {
        store.try_begin(app_id, phase)?;
        Ok(Self {
            store,
            app_id: app_id.to_string(),
        })
    }

    pub fn set_phase(&self, phase: StartPhase) {
        self.store.set_phase(self.app_id.as_str(), phase);
    }
}

impl Drop for StartInflightGuard<'_> {
    fn drop(&mut self) {
        self.store.finish(self.app_id.as_str());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_begin_is_rejected_while_inflight() {
        let store = AppStartInflight::default();
        let guard = StartInflightGuard::try_acquire(&store, "mini-data", StartPhase::Prebuilding)
            .expect("first acquire");
        let err = match StartInflightGuard::try_acquire(&store, "mini-data", StartPhase::Spawning) {
            Ok(_) => panic!("duplicate must conflict"),
            Err(existing) => existing,
        };
        assert_eq!(err.app_id, "mini-data");
        assert_eq!(err.phase, StartPhase::Prebuilding);
        drop(guard);
        StartInflightGuard::try_acquire(&store, "mini-data", StartPhase::Prebuilding)
            .expect("after drop can acquire again");
    }

    #[test]
    fn different_apps_can_begin_in_parallel() {
        let store = AppStartInflight::default();
        let a = StartInflightGuard::try_acquire(&store, "zhifa", StartPhase::Spawning).unwrap();
        let b = StartInflightGuard::try_acquire(&store, "mini-data", StartPhase::Spawning).unwrap();
        assert!(store.get("zhifa").is_some());
        assert!(store.get("mini-data").is_some());
        drop(a);
        drop(b);
    }
}

