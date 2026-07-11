use crate::EvalSlotDescriptor;

/// Host plugin trait (data plugins implement `DsPlugin`).
pub trait Plugin: Send + Sync {
    fn id(&self) -> &'static str;
}

/// Data-source plugin: parquet, query, metric eval.
pub trait DsPlugin: Plugin {
    fn materialize(&self, request: &MaterializeRequest) -> anyhow::Result<MaterializeResult>;
}

#[derive(Debug, Clone)]
pub struct MaterializeRequest {
    pub scope_key: String,
    pub workset_id: String,
    pub owner_resource_id: String,
    pub metric_ids: Vec<String>,
    pub bundle_key: String,
}

#[derive(Debug, Clone, Default)]
pub struct MaterializeResult {
    pub slots: Vec<EvalSlotDescriptor>,
}
