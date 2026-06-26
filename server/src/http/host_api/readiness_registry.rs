use super::prelude::*;
use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct HostAppReadinessState {
    pub(crate) phase: String,
    pub(crate) access_ready: bool,
    pub(crate) gate_summary: Option<ScopeGateSweepSummary>,
    pub(crate) last_error: Option<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) warning_details: Vec<PrebuildWarningReport>,
    pub(crate) warning_categories: Vec<String>,
    pub(crate) scopes: BTreeMap<String, HostScopeReadinessState>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HostScopeReadinessState {
    pub(crate) scene_id: Option<String>,
    pub(crate) target_file: Option<String>,
    pub(crate) phase: String,
    pub(crate) compile_revision: Option<String>,
    pub(crate) last_error: Option<String>,
}

pub(crate) fn host_readiness_registry() -> &'static Mutex<HostReadinessRegistry> {
    static STATUS: OnceLock<Mutex<HostReadinessRegistry>> = OnceLock::new();
    STATUS.get_or_init(|| Mutex::new(HostReadinessRegistry::default()))
}

pub(crate) fn with_registry<T>(f: impl FnOnce(&mut HostReadinessRegistry) -> T) -> Option<T> {
    host_readiness_registry()
        .lock()
        .ok()
        .map(|mut guard| f(&mut guard))
}

pub(crate) fn manifest_path_for(source_root: &Path) -> PathBuf {
    source_root.join(mei_lang_kernel::WORKSPACE_RUNTIME_WARMUP_MANIFEST_REL)
}

pub(crate) fn manifest_source_label(source_root: &Path) -> &'static str {
    if manifest_path_for(source_root).is_file() {
        "runtime_manifest"
    } else {
        "workspace_config_fallback"
    }
}

pub(crate) fn phase_ready(phase: &str) -> bool {
    matches!(phase, "ready" | "degraded" | "skipped")
}

pub(crate) fn host_started_at_ms_from_registry(snapshot: &HostReadinessRegistry) -> Option<u64> {
    snapshot
        .host_started_at_ms
        .or_else(startup_run::current_started_at_ms)
}

pub(crate) fn format_elapsed_zh(elapsed_ms: u64) -> String {
    if elapsed_ms < 1000 {
        return format!("{} 秒", elapsed_ms.max(1));
    }
    if elapsed_ms < 60_000 {
        let seconds = (elapsed_ms as f64 / 1000.0).round() as u64;
        return format!("{} 秒", seconds.max(1));
    }
    if elapsed_ms < 3_600_000 {
        let minutes = elapsed_ms / 60_000;
        let seconds = (elapsed_ms % 60_000) / 1000;
        if seconds == 0 {
            return format!("{} 分", minutes);
        }
        return format!("{} 分 {} 秒", minutes, seconds);
    }
    let hours = elapsed_ms / 3_600_000;
    let minutes = (elapsed_ms % 3_600_000) / 60_000;
    if minutes == 0 {
        return format!("{} 小时", hours);
    }
    format!("{} 小时 {} 分", hours, minutes)
}

pub(crate) fn host_warmup_in_progress() -> bool {
    let snapshot = host_readiness_registry()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    if !snapshot.host_bound {
        return true;
    }
    if snapshot.deferred_warmup_pending {
        return true;
    }
    matches!(
        snapshot.phase.as_str(),
        "starting" | "building" | "verifying"
    )
}

pub(crate) fn warmup_pending_user_message() -> String {
    let snapshot = host_readiness_registry()
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let started_at_ms = host_started_at_ms_from_registry(&snapshot);
    let elapsed_ms = started_at_ms.map(|started| {
        startup_run::now_ms_for_host_message()
            .saturating_sub(started)
    });
    let ago = elapsed_ms
        .map(format_elapsed_zh)
        .unwrap_or_else(|| "刚刚".to_string());
    let detail = if snapshot.deferred_warmup_pending {
        "后台仍在装载 deferred 指标"
    } else if matches!(
        snapshot.phase.as_str(),
        "building" | "verifying" | "bound"
    ) {
        "后台正在编译与预热"
    } else if !snapshot.access_ready {
        "启动预热尚未完成"
    } else {
        "访问态产物仍在装载"
    };
    format!(
        "系统于 {ago} 前刚刚启动，{detail}，该指标尚未装载，请稍候刷新页面。"
    )
}

pub(crate) fn is_warmup_transient_runtime_error(message: &str) -> bool {
    let text = message.trim();
    text.contains("not found in active scene resources")
        || text.contains("missing strict AOT metric result artifact")
        || text.contains("requires prebuilt access artifacts on access-only host")
        || text.contains("该指标尚未装载")
}

pub(crate) fn normalize_scope_key(scene_id: Option<&str>, target_file: Option<&str>) -> String {
    format!(
        "{}|{}",
        scene_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(""),
        target_file
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("")
    )
}

pub(crate) fn scope_response_from_state(scope: HostScopeReadinessState) -> HostScopeReadinessResponse {
    HostScopeReadinessResponse {
        scene_id: scope.scene_id,
        target_file: scope.target_file,
        phase: if scope.phase.trim().is_empty() {
            "missing".to_string()
        } else {
            scope.phase
        },
        compile_revision: scope.compile_revision,
        last_error: scope.last_error,
    }
}

pub(crate) fn app_response(app_id: String, state: HostAppReadinessState) -> HostAppReadinessResponse {
    let scopes = state
        .scopes
        .into_values()
        .map(scope_response_from_state)
        .collect::<Vec<_>>();
    let ready_scope_count = scopes
        .iter()
        .filter(|scope| phase_ready(scope.phase.as_str()))
        .count();
    let failed_scope_count = scopes
        .iter()
        .filter(|scope| matches!(scope.phase.as_str(), "failed"))
        .count();
    HostAppReadinessResponse {
        app_id,
        ready: state.access_ready,
        access_ready: state.access_ready,
        phase: if state.phase.trim().is_empty() {
            "pending".to_string()
        } else {
            state.phase
        },
        gate_summary: state.gate_summary,
        last_error: state.last_error,
        warnings: state.warnings,
        warning_details: state.warning_details,
        warning_categories: state.warning_categories,
        compile_scope_count: scopes.len(),
        ready_scope_count,
        failed_scope_count,
        scopes,
    }
}

