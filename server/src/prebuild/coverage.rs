use super::prelude::*;
use super::*;

pub(crate) struct CoverageState {
    pub(crate) metric_response_jobs: ArtifactSingleflightState,
    pub(crate) metric_dataframe_jobs: ArtifactSingleflightState,
    pub(crate) metric_response_exact: Arc<Mutex<BTreeMap<String, LoadedMetricResponseArtifact>>>,
    pub(crate) metric_response_shared: Arc<Mutex<BTreeMap<String, LoadedMetricResponseArtifact>>>,
    pub(crate) metric_dataframe_exact: Arc<Mutex<BTreeMap<String, DatasetQueryResult>>>,
    pub(crate) metric_dataframe_shared: Arc<Mutex<BTreeMap<String, DatasetQueryResult>>>,
    pub(crate) diagnostics: Arc<PrebuildDiagnostics>,
    /// MetricDefBundle revisions captured before compile (MCG P1 skip).
    pub(crate) pre_mcg_bundle_revisions: BTreeMap<String, String>,
    pub(crate) source_root: Option<std::path::PathBuf>,
    pub(crate) app_id: Option<String>,
}

impl Default for CoverageState {
    fn default() -> Self {
        Self {
            metric_response_jobs: ArtifactSingleflightState::default(),
            metric_dataframe_jobs: ArtifactSingleflightState::default(),
            metric_response_exact: Arc::new(Mutex::new(BTreeMap::new())),
            metric_response_shared: Arc::new(Mutex::new(BTreeMap::new())),
            metric_dataframe_exact: Arc::new(Mutex::new(BTreeMap::new())),
            metric_dataframe_shared: Arc::new(Mutex::new(BTreeMap::new())),
            diagnostics: Arc::new(PrebuildDiagnostics::default()),
            pre_mcg_bundle_revisions: BTreeMap::new(),
            source_root: None,
            app_id: None,
        }
    }
}

#[derive(Default)]
pub(crate) struct ArtifactSingleflightState {
    pub(crate) state: Mutex<ArtifactSingleflightInner>,
    pub(crate) ready: Condvar,
}

#[derive(Default)]
pub(crate) struct ArtifactSingleflightInner {
    pub(crate) inflight: BTreeSet<String>,
    pub(crate) completed: BTreeSet<String>,
}

pub(crate) enum ArtifactReservation {
    Reserved,
    Completed,
}

impl ArtifactSingleflightState {
    pub(crate) fn wait_or_reserve(&self, key: &str) -> ArtifactReservation {
        let mut state = self.state.lock().expect("lock prebuild singleflight");
        loop {
            if state.completed.contains(key) {
                return ArtifactReservation::Completed;
            }
            if state.inflight.insert(key.to_string()) {
                return ArtifactReservation::Reserved;
            }
            state = self.ready.wait(state).expect("wait prebuild singleflight");
        }
    }

    pub(crate) fn finish(&self, key: &str, success: bool) {
        let mut state = self.state.lock().expect("lock prebuild singleflight");
        state.inflight.remove(key);
        if success {
            state.completed.insert(key.to_string());
        }
        self.ready.notify_all();
    }

    pub(crate) fn clear(&self) {
        let mut state = self.state.lock().expect("lock prebuild singleflight");
        state.inflight.clear();
        state.completed.clear();
    }
}

impl CoverageState {
    pub(crate) fn metric_response_exact(&self, key: &str) -> Option<LoadedMetricResponseArtifact> {
        self.metric_response_exact
            .lock()
            .expect("lock prebuild response exact cache")
            .get(key)
            .cloned()
    }

    pub(crate) fn metric_response_shared(&self, key: &str) -> Option<LoadedMetricResponseArtifact> {
        self.metric_response_shared
            .lock()
            .expect("lock prebuild response shared cache")
            .get(key)
            .cloned()
    }

    pub(crate) fn store_metric_response_exact(&self, key: &str, artifact: &LoadedMetricResponseArtifact) {
        self.metric_response_exact
            .lock()
            .expect("lock prebuild response exact cache")
            .insert(key.to_string(), artifact.clone());
    }

    pub(crate) fn store_metric_response_shared(&self, key: &str, artifact: &LoadedMetricResponseArtifact) {
        self.metric_response_shared
            .lock()
            .expect("lock prebuild response shared cache")
            .insert(key.to_string(), artifact.clone());
    }

    pub(crate) fn metric_dataframe_exact(&self, key: &str) -> Option<DatasetQueryResult> {
        self.metric_dataframe_exact
            .lock()
            .expect("lock prebuild dataframe exact cache")
            .get(key)
            .cloned()
    }

    pub(crate) fn metric_dataframe_shared(&self, key: &str) -> Option<DatasetQueryResult> {
        self.metric_dataframe_shared
            .lock()
            .expect("lock prebuild dataframe shared cache")
            .get(key)
            .cloned()
    }

    pub(crate) fn store_metric_dataframe_exact(&self, key: &str, result: &DatasetQueryResult) {
        self.metric_dataframe_exact
            .lock()
            .expect("lock prebuild dataframe exact cache")
            .insert(key.to_string(), result.clone());
    }

    pub(crate) fn store_metric_dataframe_shared(&self, key: &str, result: &DatasetQueryResult) {
        self.metric_dataframe_shared
            .lock()
            .expect("lock prebuild dataframe shared cache")
            .insert(key.to_string(), result.clone());
    }

    pub(crate) fn clear(&self) {
        self.metric_response_exact
            .lock()
            .expect("lock prebuild response exact cache")
            .clear();
        self.metric_response_shared
            .lock()
            .expect("lock prebuild response shared cache")
            .clear();
        self.metric_dataframe_exact
            .lock()
            .expect("lock prebuild dataframe exact cache")
            .clear();
        self.metric_dataframe_shared
            .lock()
            .expect("lock prebuild dataframe shared cache")
            .clear();
        self.metric_response_jobs.clear();
        self.metric_dataframe_jobs.clear();
    }

    pub(crate) fn active_mcg_bundle_revisions(&self) -> BTreeMap<String, String> {
        let mut revisions = self.pre_mcg_bundle_revisions.clone();
        if let (Some(source_root), Some(app_id)) = (
            self.source_root.as_deref(),
            self.app_id.as_deref(),
        ) {
            revisions.extend(crate::graph::load_mcg_bundle_revisions(source_root, app_id));
        }
        revisions
    }
}
