use crate::model::CompiledApp;

pub(super) fn world_file_symbol_id(compiled: &CompiledApp, file: &str) -> String {
    compiled
        .world_semantic_by_file
        .get(file)
        .and_then(|index| index.world_id.as_deref())
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| file.to_string())
}
