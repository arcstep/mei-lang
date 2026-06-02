//! Compile-time semantics generation token.
//!
//! Bump when kernel outputs change without workspace file changes (e.g. analysis
//! contracts, projection assembly). Server compile cache keys include this value.

/// Bump to invalidate in-process compile caches after kernel semantic changes.
pub const COMPILE_SEMANTICS_GENERATION: &str = "2026-06-02-projection-dataframe-blocks";
