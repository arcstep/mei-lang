use std::collections::BTreeMap;

/// Result of importing a `.meibundle` into MCG/MRG + CAS.
#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub app_id: String,
    pub block_count: usize,
    pub cas_upserts: usize,
    pub mcg_nodes: usize,
    pub registry_revision: String,
    pub index_by_kind: BTreeMap<String, usize>,
    pub warnings: Vec<String>,
}
