mod account;
mod authorize;
mod cli;
pub mod cli_args;
mod crypto;
mod http;
mod landing;
mod runtime;
mod shell_chrome;
mod state;
mod types;
mod workspace_users;

#[cfg(test)]
mod tests;

pub use account::account_view_for_principal;
pub use authorize::{
    auth_middleware, authorize_next_path, authorize_path, prepare_auth_for_serve,
    sanitize_next_path,
};
pub use cli::{
    print_json_output, read_password_from_stdin, run_auth_command, run_legacy_auth_command,
};
pub use crypto::{
    clear_cookie_header_value, cookie_header_value, generate_temporary_password, hash_password,
};
pub use http::{
    account_change_password_page, auth_change_password, auth_login, auth_logout, auth_public_key,
    auth_refresh, auth_session, login_page, logout_page,
};
pub use landing::{access_landing_location, filter_apps_for_principal, v2_index_landing_location};
pub use runtime::{load_auth_runtime, normalize_id, SESSION_REFRESH_LEAD_SECONDS};
pub use shell_chrome::{
    host_shell_body_theme_style, host_starting_html_response, html_escape, render_auth_card_page,
    render_host_shell_footer_for_source_root, render_startup_warming_main_html,
    startup_failed_html_response, startup_warming_html_response, startup_warming_poll_script,
};
pub use state::AuthServeState;
pub use types::{AuthEnforcement, AuthPrincipal, AuthRole, AuthRuntime};
pub use workspace_users::{
    ensure_workspace_auth_base, rotate_workspace_key_pair, set_workspace_user_disabled,
    update_workspace_user_password, upsert_workspace_user,
};
