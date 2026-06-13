mod authorize;
mod crypto;
mod runtime;
mod types;
mod workspace_users;

#[cfg(test)]
mod tests;

pub use authorize::{
    auth_middleware, authorize_next_path, prepare_auth_for_serve, sanitize_next_path,
};
pub use crypto::{
    clear_cookie_header_value, cookie_header_value, generate_temporary_password, hash_password,
};
pub use runtime::{load_auth_runtime, normalize_id, SESSION_REFRESH_LEAD_SECONDS};
pub use types::{AuthEnforcement, AuthPrincipal, AuthRole, AuthRuntime};
pub use workspace_users::{
    ensure_workspace_auth_base, rotate_workspace_key_pair, set_workspace_user_disabled,
    update_workspace_user_password, upsert_workspace_user,
};
