use crate::types::stable_hash;

pub fn compute_client_revision(
    slot_revision: &str,
    content_hash: &str,
    data_generation: &str,
) -> String {
    stable_hash(&format!(
        "{slot_revision}\n{content_hash}\n{data_generation}"
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmupTier {
    Disk,
    Memory,
    Client,
    All,
}

impl WarmupTier {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_lowercase().as_str() {
            "memory" => Self::Memory,
            "client" => Self::Client,
            "all" => Self::All,
            _ => Self::Disk,
        }
    }

    pub fn wants_disk(self) -> bool {
        matches!(self, Self::Disk | Self::Memory | Self::Client | Self::All)
    }

    pub fn wants_memory(self) -> bool {
        matches!(self, Self::Memory | Self::All)
    }

    pub fn wants_client(self) -> bool {
        matches!(self, Self::Client | Self::All)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_client_revision_is_stable_for_same_inputs() {
        let first = compute_client_revision("slot", "hash", "gen");
        let second = compute_client_revision("slot", "hash", "gen");
        assert_eq!(first, second);
    }

    #[test]
    fn compute_client_revision_changes_when_data_generation_changes() {
        let first = compute_client_revision("slot", "hash", "gen-a");
        let second = compute_client_revision("slot", "hash", "gen-b");
        assert_ne!(first, second);
    }
}
