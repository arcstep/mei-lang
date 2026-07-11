//! Legacy in-memory HTML cache for unified `/view` SSR (abolished).
//! Kept as a no-op clear hook so prebuild/reload hygiene call sites stay stable.

pub fn clear_for_app(_app_id: &str) {}
