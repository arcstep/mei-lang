use std::{path::PathBuf, sync::Arc};

use crate::types::AuthEnforcement;

#[derive(Clone)]
pub struct AuthServeState {
    pub source_root: Arc<PathBuf>,
    pub auth_enforcement: AuthEnforcement,
}

impl AuthServeState {
    pub fn new(source_root: PathBuf, auth_enforcement: AuthEnforcement) -> Self {
        Self {
            source_root: Arc::new(source_root),
            auth_enforcement,
        }
    }
}
