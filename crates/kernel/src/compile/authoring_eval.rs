use std::path::Path;
use std::sync::Arc;

use anyhow::Result;

use crate::eval::{active_authoring_helpers, push_authoring_helpers, AuthoringEvalGuard};
use crate::mei_config::{resolve_authoring_helpers, AuthoringHelpers};

/// Run compile/eval work with workspace authoring helpers installed on this thread.
#[allow(dead_code)]
pub fn with_authoring_eval_context<T>(
    source_root: &Path,
    work: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let helpers = resolve_authoring_helpers(source_root)?;
    let _guard = push_authoring_helpers(helpers);
    work()
}

pub fn install_authoring_eval_context(source_root: &Path) -> Result<AuthoringEvalGuard> {
    let helpers = resolve_authoring_helpers(source_root)?;
    Ok(push_authoring_helpers(helpers))
}

/// Helpers for the current compile, suitable for cloning into parallel workers.
pub fn shared_authoring_helpers_for_compile(source_root: &Path) -> Arc<AuthoringHelpers> {
    active_authoring_helpers()
        .map(Arc::new)
        .unwrap_or_else(|| {
            Arc::new(resolve_authoring_helpers(source_root).unwrap_or_default())
        })
}

/// Install helpers on a worker thread when fingerprint is non-empty.
pub fn install_shared_authoring_guard(
    helpers: &AuthoringHelpers,
) -> Option<AuthoringEvalGuard> {
    if helpers.fingerprint.is_empty() {
        None
    } else {
        Some(push_authoring_helpers(helpers.clone()))
    }
}
