use std::sync::OnceLock;

use mei_host_auth::html_escape;
use mei_lang_kernel::{AdminPageProgram, PageProgram};
use sha2::{Digest, Sha256};

mod home_generated {
    include!(concat!(env!("OUT_DIR"), "/host_home_page_pack.rs"));
}

mod runtime_generated {
    include!(concat!(env!("OUT_DIR"), "/host_runtime_page_pack.rs"));
}

#[derive(Debug, Clone)]
pub(crate) struct HostPagePack {
    pub pack_id: String,
    pub digest: String,
    pub page_program: PageProgram,
    pub admin_page_program: Option<AdminPageProgram>,
    pub aot_body_template: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HostPagePackError {
    Missing,
    InvalidMetadata,
    InvalidDigest,
    InvalidTemplate,
}

impl HostPagePackError {
    fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::InvalidMetadata => "invalid-metadata",
            Self::InvalidDigest => "invalid-digest",
            Self::InvalidTemplate => "invalid-template",
        }
    }
}

pub(crate) fn home_page_pack() -> &'static HostPagePack {
    static PACK: OnceLock<HostPagePack> = OnceLock::new();
    PACK.get_or_init(|| HostPagePack {
        pack_id: home_generated::HOME_PAGE_PACK_ID.to_string(),
        digest: home_generated::HOME_PAGE_PACK_DIGEST.to_string(),
        page_program: PageProgram::from_scene_ref(
            home_generated::HOME_PAGE_ID,
            Some(home_generated::HOME_PAGE_TITLE.to_string()),
            home_generated::HOME_PAGE_SOURCE_ANCHOR,
            home_generated::HOME_PAGE_SCENE_REF,
        ),
        admin_page_program: None,
        aot_body_template: home_generated::HOME_PAGE_PACK_TEMPLATE.to_string(),
    })
}

pub(crate) fn runtime_page_pack() -> &'static HostPagePack {
    static PACK: OnceLock<HostPagePack> = OnceLock::new();
    PACK.get_or_init(|| HostPagePack {
        pack_id: runtime_generated::RUNTIME_PAGE_PACK_ID.to_string(),
        digest: runtime_generated::RUNTIME_PAGE_PACK_DIGEST.to_string(),
        page_program: PageProgram::from_scene_ref(
            runtime_generated::RUNTIME_PAGE_ID,
            Some(runtime_generated::RUNTIME_PAGE_TITLE.to_string()),
            runtime_generated::RUNTIME_PAGE_SOURCE_ANCHOR,
            runtime_generated::RUNTIME_PAGE_SCENE_REF,
        ),
        admin_page_program: None,
        aot_body_template: runtime_generated::RUNTIME_PAGE_PACK_TEMPLATE.to_string(),
    })
}

fn canonical_page_pack_payload(pack: &HostPagePack) -> String {
    let admin_resource_id = pack
        .admin_page_program
        .as_ref()
        .map(|program| program.resource_id.as_str())
        .unwrap_or_default();
    format!(
        "host-page-pack-v1\npack_id:{}\npage_id:{}\ntitle:{}\nsource_anchor:{}\nsurface:{}\nscene_ref:{}\nadmin_resource_id:{}\naot_body:\n{}",
        pack.pack_id,
        pack.page_program.page_id,
        pack.page_program.title.as_deref().unwrap_or_default(),
        pack.page_program.source_anchor,
        pack.page_program.surface.as_str(),
        pack.page_program.root.scene_ref(),
        admin_resource_id,
        pack.aot_body_template,
    )
}

pub(crate) fn digest_for_page_pack(pack: &HostPagePack) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(canonical_page_pack_payload(pack).as_bytes())
    )
}

fn validate_page_pack<'a>(
    pack: Option<&'a HostPagePack>,
    expected: &HostPagePack,
) -> Result<&'a HostPagePack, HostPagePackError> {
    let pack = pack.ok_or(HostPagePackError::Missing)?;
    let metadata_valid = pack.pack_id == expected.pack_id
        && pack.page_program.page_id == expected.page_program.page_id
        && pack.page_program.title == expected.page_program.title
        && pack.page_program.source_anchor == expected.page_program.source_anchor
        && pack.page_program.surface.as_str() == "document"
        && pack.page_program.root.scene_ref() == expected.page_program.root.scene_ref()
        && pack.admin_page_program.is_none();
    if !metadata_valid {
        return Err(HostPagePackError::InvalidMetadata);
    }
    if pack.digest != expected.digest || digest_for_page_pack(pack) != expected.digest {
        return Err(HostPagePackError::InvalidDigest);
    }
    Ok(pack)
}

pub(crate) fn validate_home_page_pack(
    pack: Option<&HostPagePack>,
) -> Result<&HostPagePack, HostPagePackError> {
    validate_page_pack(pack, home_page_pack())
}

pub(crate) fn validate_runtime_page_pack(
    pack: Option<&HostPagePack>,
) -> Result<&HostPagePack, HostPagePackError> {
    validate_page_pack(pack, runtime_page_pack())
}

fn fill_page_pack_slots(
    pack: &HostPagePack,
    slots: &[(&str, String)],
) -> Result<String, HostPagePackError> {
    let mut html = pack.aot_body_template.clone();
    for (slot, value) in slots {
        if html.matches(slot).count() != 1 {
            return Err(HostPagePackError::InvalidTemplate);
        }
        html = html.replace(slot, value.as_str());
    }
    if html.contains("{{mei:") {
        return Err(HostPagePackError::InvalidTemplate);
    }
    Ok(html)
}

pub(crate) fn render_home_page_body(
    pack: Option<&HostPagePack>,
    workspace_line: &str,
    app_cards: &str,
) -> Result<String, HostPagePackError> {
    let pack = validate_home_page_pack(pack)?;
    fill_page_pack_slots(
        pack,
        &[
            ("{{mei:pack_id}}", html_escape(pack.pack_id.as_str())),
            ("{{mei:digest}}", html_escape(pack.digest.as_str())),
            (
                "{{mei:surface}}",
                html_escape(pack.page_program.surface.as_str()),
            ),
            ("{{mei:workspace_line}}", workspace_line.to_string()),
            ("{{mei:app_cards}}", app_cards.to_string()),
        ],
    )
}

pub(crate) fn render_runtime_page_body(
    pack: Option<&HostPagePack>,
    host_tools: &str,
    runtime_control: &str,
) -> Result<String, HostPagePackError> {
    let pack = validate_runtime_page_pack(pack)?;
    fill_page_pack_slots(
        pack,
        &[
            ("{{mei:pack_id}}", html_escape(pack.pack_id.as_str())),
            ("{{mei:digest}}", html_escape(pack.digest.as_str())),
            (
                "{{mei:surface}}",
                html_escape(pack.page_program.surface.as_str()),
            ),
            ("{{mei:host_tools}}", host_tools.to_string()),
            ("{{mei:runtime_control}}", runtime_control.to_string()),
        ],
    )
}

pub(crate) fn render_native_recovery_html(error: HostPagePackError) -> String {
    format!(
        r#"<!DOCTYPE html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>MeiLang Recovery</title></head><body data-mei-native-recovery="host-page-pack" data-recovery-reason="{reason}"><main><h1>MeiLang Host 恢复页</h1><p>Host 页面资源暂时不可用，请从原生入口继续。</p><nav aria-label="恢复入口"><a href="/home">首页</a> <a href="/runtime">运行控制中心</a> <a href="/login">登录</a></nav></main></body></html>"#,
        reason = error.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_page_pack_digest_and_metadata_are_stable() {
        let pack = home_page_pack();
        assert_eq!(pack.pack_id, "host.home");
        assert_eq!(pack.page_program.page_id, "home");
        assert_eq!(pack.page_program.surface.as_str(), "document");
        assert_eq!(
            pack.page_program.source_anchor,
            "host://pagepacks/home.page.mdx"
        );
        assert_eq!(pack.page_program.root.scene_ref(), "host/home");
        assert!(pack.admin_page_program.is_none());
        assert_eq!(pack.digest, digest_for_page_pack(pack));
        assert_eq!(pack.digest.len(), "sha256:".len() + 64);
        assert!(std::ptr::eq(pack, home_page_pack()));
    }

    #[test]
    fn runtime_page_pack_digest_and_metadata_are_stable() {
        let pack = runtime_page_pack();
        assert_eq!(pack.pack_id, "host.runtime");
        assert_eq!(pack.page_program.page_id, "runtime");
        assert_eq!(pack.page_program.surface.as_str(), "document");
        assert_eq!(
            pack.page_program.source_anchor,
            "host://pagepacks/runtime.page.mdx"
        );
        assert_eq!(pack.page_program.root.scene_ref(), "host/runtime");
        assert!(!pack.aot_body_template.contains("<script"));
        assert_eq!(pack.digest, digest_for_page_pack(pack));
        assert!(std::ptr::eq(pack, runtime_page_pack()));
    }

    #[test]
    fn host_page_pack_missing_or_invalid_uses_native_recovery() {
        for error in [
            validate_home_page_pack(None).expect_err("missing pack"),
            {
                let mut invalid = home_page_pack().clone();
                invalid.digest = "sha256:invalid".to_string();
                validate_home_page_pack(Some(&invalid)).expect_err("invalid pack")
            },
            validate_runtime_page_pack(None).expect_err("missing runtime pack"),
            {
                let mut invalid = runtime_page_pack().clone();
                invalid.aot_body_template.push_str("<!-- corrupt -->");
                validate_runtime_page_pack(Some(&invalid)).expect_err("invalid runtime pack")
            },
        ] {
            let html = render_native_recovery_html(error);
            assert!(html.contains("data-mei-native-recovery=\"host-page-pack\""));
            assert!(html.contains("href=\"/home\""));
            assert!(html.contains("href=\"/runtime\""));
            assert!(html.contains("href=\"/login\""));
            assert!(!html.contains("/app-assets/"));
            assert!(!html.contains("<script"));
        }
    }

    #[test]
    fn runtime_page_body_fills_mount_slots() {
        let html = render_runtime_page_body(
            Some(runtime_page_pack()),
            r#"<nav data-runtime-tools>tools</nav>"#,
            r#"<div data-host-runtime-control-center>control</div>"#,
        )
        .expect("runtime body");
        assert!(html.contains(r#"data-mei-pagepack="host.runtime""#));
        assert!(html.contains(r#"data-mei-page-surface="document""#));
        assert!(html.contains("data-runtime-tools"));
        assert!(html.contains("data-host-runtime-control-center"));
        assert!(!html.contains("{{mei:"));
    }
}
