use std::collections::{BTreeMap, BTreeSet};

use crate::SceneContract;

pub(in crate::runtime::engine) fn base_seed(seed: u64) -> u64 {
    seed.max(1)
}

pub(in crate::runtime::engine) fn next_seed(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *seed
}

pub(in crate::runtime::engine) fn choose_slot(
    seed: &mut u64,
    candidates: &[String],
    used: &BTreeSet<String>,
) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    let available = candidates
        .iter()
        .filter(|slot| !used.contains(*slot))
        .cloned()
        .collect::<Vec<_>>();
    let pool = if available.is_empty() {
        candidates.to_vec()
    } else {
        available
    };
    let index = (next_seed(seed) as usize) % pool.len();
    pool.get(index).cloned()
}

pub(in crate::runtime::engine) fn base_statuses(contract: &SceneContract) -> BTreeMap<String, String> {
    contract
        .world
        .as_ref()
        .map(|world| {
            world
                .entities
                .iter()
                .filter_map(|entity| {
                    entity
                        .status
                        .clone()
                        .map(|status| (entity.id.clone(), status))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(in crate::runtime::engine) fn base_flags(contract: &SceneContract) -> BTreeMap<String, bool> {
    let mut flags = BTreeMap::new();
    if let Some(world) = &contract.world {
        for entity in &world.entities {
            if let Some(map) = entity.flags.as_object() {
                for (key, value) in map {
                    if let Some(flag) = value.as_bool() {
                        flags.insert(format!("{}.{}", entity.id, key), flag);
                    }
                }
            }
        }
    }
    flags
}
