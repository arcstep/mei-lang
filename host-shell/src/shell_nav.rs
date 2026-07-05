//! Shared shell navigation links for host light pages.

use mei_host_auth::html_escape;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellNavItem {
    Home,
    Config,
    Upload,
    Runtime,
    Mcg,
}

pub fn render_shell_nav_html(active: ShellNavItem) -> String {
    const ITEMS: &[(&str, &str, ShellNavItem)] = &[
        ("首页", "/home", ShellNavItem::Home),
        ("配置", "/config", ShellNavItem::Config),
        ("上传", "/upload", ShellNavItem::Upload),
        ("运行", "/runtime", ShellNavItem::Runtime),
        ("MCG", "/mcg", ShellNavItem::Mcg),
    ];
    let links = ITEMS
        .iter()
        .map(|(label, href, item)| {
            let class = if *item == active {
                "mei-host-shell__nav-link is-active"
            } else {
                "mei-host-shell__nav-link"
            };
            format!(
                r#"<a class="{class}" href="{href}">{label}</a>"#,
                class = class,
                href = html_escape(href),
                label = html_escape(label),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!(r#"<nav class="mei-host-shell__nav" aria-label="工作区导航">{links}</nav>"#)
}
