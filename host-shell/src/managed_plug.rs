//! Retired Host-managed `mei-plug-ds` sidecars.
//!
//! Prefer `mei-app-runtime` (embedded DS). Standalone plug-ds processes are no
//! longer spawned; see [`crate::legacy_compat::apps_needing_managed_plug_ds`].

use std::collections::{BTreeMap, BTreeSet};

/// Empty pool retained for Host teardown / profile-switch API compatibility.
pub struct ManagedPlugDsPool {
    pub endpoints: BTreeMap<String, String>,
}

impl ManagedPlugDsPool {
    pub async fn shutdown(&mut self) -> anyhow::Result<()> {
        self.endpoints.clear();
        Ok(())
    }
}

/// No-op when every app is covered by app-runtime; otherwise returns a clear error.
///
/// Standalone `mei-plug-ds` is retired — never spawn sidecars.
pub async fn spawn_managed_plug_ds_pool(
    _workspace_root: &std::path::Path,
    app_ids: &[String],
    covered_by_runtime: &BTreeSet<String>,
) -> anyhow::Result<ManagedPlugDsPool> {
    let needing = crate::legacy_compat::apps_needing_managed_plug_ds(app_ids, covered_by_runtime);
    if needing.is_empty() {
        if !app_ids.is_empty() {
            tracing::info!(
                skipped = app_ids.len(),
                "skipping managed plug-ds; all target apps covered by app-runtime routes"
            );
        }
        return Ok(ManagedPlugDsPool {
            endpoints: BTreeMap::new(),
        });
    }
    tracing::error!(
        apps = ?needing,
        "standalone mei-plug-ds is retired; apps without mei-app-runtime coverage have no data plane"
    );
    anyhow::bail!(
        "standalone mei-plug-ds is retired; cover apps with mei-app-runtime (missing: {})",
        needing.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::legacy_compat::apps_needing_managed_plug_ds;

    #[test]
    fn skip_set_filters_spawn_targets() {
        let covered = BTreeSet::from(["mini-data".to_string(), "a".to_string()]);
        let needing =
            apps_needing_managed_plug_ds(&["mini-data".into(), "a".into(), "b".into()], &covered);
        assert_eq!(needing, vec!["b".to_string()]);
    }

    #[tokio::test]
    async fn spawn_pool_skips_all_when_fully_covered() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let covered = BTreeSet::from(["mini-data".to_string()]);
        let pool = spawn_managed_plug_ds_pool(tmp.path(), &["mini-data".into()], &covered)
            .await
            .expect("empty pool");
        assert!(pool.endpoints.is_empty());
    }

    #[tokio::test]
    async fn spawn_pool_errors_when_runtime_coverage_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let covered = BTreeSet::new();
        let err = match spawn_managed_plug_ds_pool(tmp.path(), &["orphan-app".into()], &covered).await
        {
            Ok(_) => panic!("must refuse standalone plug-ds"),
            Err(err) => err,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("retired") && msg.contains("orphan-app"),
            "unexpected error: {msg}"
        );
    }
}
