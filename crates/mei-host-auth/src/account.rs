use mei_lang_app::HostAccountView;

use crate::types::AuthPrincipal;

pub fn account_view_for_principal(principal: Option<&AuthPrincipal>) -> Option<HostAccountView> {
    principal.map(|principal| HostAccountView {
        logged_in: true,
        username: principal.username.clone(),
        profile: principal.profile.clone(),
        role: principal.role_slug().to_string(),
        capabilities: principal.capabilities(),
    })
}
