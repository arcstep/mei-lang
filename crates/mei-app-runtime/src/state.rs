use std::sync::{Arc, RwLock};

use mei_host_core::{
    BundleRef, ConfigSnapshot, HostContext, InstancePhase, InstanceRevisions, InstanceSpec,
    RuntimeContext, SCHEMA_INSTANCE_SPEC_V1,
};
use mei_lang_kernel::{RuntimeMode, RuntimePlan};
use serde_json::Value;

/// Snapshot exposed by `/api/app-runtime/ready` and `/meta`.
#[derive(Debug, Clone)]
pub struct ReadySnapshot {
    pub phase: InstancePhase,
    pub ready: bool,
    pub last_error: Option<String>,
    pub revisions: InstanceRevisions,
}

#[derive(Clone)]
pub struct AppRuntimeServeState {
    pub host: HostContext,
    pub runtime: RuntimeContext,
    pub spec: InstanceSpec,
    pub token: String,
    pub phase: Arc<RwLock<ReadySnapshot>>,
}

pub type SharedRuntimeState = Arc<AppRuntimeServeState>;

impl AppRuntimeServeState {
    pub fn new(host: HostContext, spec: InstanceSpec, token: impl Into<String>) -> Self {
        let runtime = RuntimeContext::from_instance_spec(host.workspace_root.as_path(), &spec);
        Self {
            host,
            runtime,
            spec,
            token: token.into(),
            phase: Arc::new(RwLock::new(ReadySnapshot {
                phase: InstancePhase::Launching,
                ready: false,
                last_error: None,
                revisions: InstanceRevisions::default(),
            })),
        }
    }

    pub fn shared(self) -> SharedRuntimeState {
        Arc::new(self)
    }

    pub fn set_phase(&self, phase: InstancePhase) {
        if let Ok(mut guard) = self.phase.write() {
            guard.phase = phase;
            guard.ready = matches!(phase, InstancePhase::Ready);
            if !matches!(phase, InstancePhase::Failed) {
                guard.last_error = None;
            }
        }
    }

    pub fn set_failed(&self, error: impl Into<String>) {
        if let Ok(mut guard) = self.phase.write() {
            guard.phase = InstancePhase::Failed;
            guard.ready = false;
            guard.last_error = Some(error.into());
        }
    }

    pub fn set_revisions(&self, revisions: InstanceRevisions) {
        if let Ok(mut guard) = self.phase.write() {
            guard.revisions = revisions;
        }
    }

    pub fn snapshot(&self) -> ReadySnapshot {
        self.phase
            .read()
            .map(|guard| guard.clone())
            .unwrap_or(ReadySnapshot {
                phase: InstancePhase::Failed,
                ready: false,
                last_error: Some("phase lock poisoned".to_string()),
                revisions: InstanceRevisions::default(),
            })
    }

    pub fn app_id(&self) -> &str {
        self.spec.app_id.as_str()
    }

    pub fn generation(&self) -> &str {
        self.spec.bundle.generation.as_str()
    }

    pub fn instance_id(&self) -> &str {
        self.spec.instance_id.as_str()
    }

    pub fn spec_digest(&self) -> String {
        self.spec.spec_digest()
    }
}

/// Resolve or synthesize [`InstanceSpec`] from CLI inputs.
pub fn resolve_instance_spec(
    workspace: &std::path::Path,
    app_id: &str,
    instance_id: &str,
    generation: Option<&str>,
    instance_spec_path: Option<&std::path::Path>,
) -> anyhow::Result<InstanceSpec> {
    if let Some(path) = instance_spec_path {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("read --instance-spec {}: {e}", path.display()))?;
        let spec: InstanceSpec = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("parse --instance-spec {}: {e}", path.display()))?;
        if spec.app_id != app_id {
            anyhow::bail!(
                "instance-spec appId `{}` does not match --app `{}`",
                spec.app_id,
                app_id
            );
        }
        if spec.instance_id != instance_id {
            anyhow::bail!(
                "instance-spec instanceId `{}` does not match --instance-id `{}`",
                spec.instance_id,
                instance_id
            );
        }
        if let Some(gen) = generation.map(str::trim).filter(|v| !v.is_empty()) {
            if spec.bundle.generation != gen {
                anyhow::bail!(
                    "instance-spec generation `{}` does not match --generation `{}`",
                    spec.bundle.generation,
                    gen
                );
            }
        }
        return Ok(spec);
    }

    let app_root = mei_lang_kernel::resolve_app_root(workspace, app_id);
    let generation = match generation.map(str::trim).filter(|v| !v.is_empty()) {
        Some(gen) => gen.to_string(),
        None => mei_lang_kernel::resolve_app_build_generation_from_current(app_root.as_path())
            .unwrap_or_else(|_| "current".to_string()),
    };
    let runtime_plan = load_runtime_plan(workspace).unwrap_or(RuntimePlan {
        default_mode: RuntimeMode::Lazy,
        apps: Default::default(),
    });
    let bundle_path = format!("apps/{app_id}/env/{generation}");
    Ok(InstanceSpec {
        schema_version: SCHEMA_INSTANCE_SPEC_V1.to_string(),
        instance_id: instance_id.to_string(),
        app_id: app_id.to_string(),
        bundle: BundleRef {
            generation,
            bundle_path,
            digest: None,
            toolchain_version: None,
            config_digest: None,
        },
        config_snapshot: ConfigSnapshot {
            profile_id: "runtime".to_string(),
            profile_revision: "0".to_string(),
            profile_file: String::new(),
            runtime_plan,
            default_app: Some(app_id.to_string()),
        },
        runtime_abi: env!("CARGO_PKG_VERSION").to_string(),
        data_mode_ceiling: None,
    })
}

fn load_runtime_plan(workspace: &std::path::Path) -> Option<RuntimePlan> {
    let path = workspace.join("deploy/applied/runtime-plan.json");
    if !path.is_file() {
        return None;
    }
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    serde_json::from_value(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn synthesize_spec_when_no_json() {
        let tmp = tempdir().expect("tempdir");
        let app = tmp.path().join("apps/demo");
        std::fs::create_dir_all(app.join("env")).expect("mkdir");
        std::os::unix::fs::symlink("WS-20260712.1", app.join("env/current")).expect("symlink");
        let spec = resolve_instance_spec(tmp.path(), "demo", "inst-1", None, None).expect("spec");
        assert_eq!(spec.app_id, "demo");
        assert_eq!(spec.instance_id, "inst-1");
        assert_eq!(spec.bundle.generation, "WS-20260712.1");
    }
}
