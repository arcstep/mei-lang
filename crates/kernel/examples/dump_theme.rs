fn main() {
    let app = std::path::PathBuf::from(
        "/Users/xuehongwei/codeup/mei-projects/workspaces/ws-demo-v2/apps/zhifa",
    );
    let cfg = mei_lang_kernel::load_mei_config_for_app(&app, None);
    let theme = cfg.ops.themes.get("cockpit").cloned().unwrap_or_default();
    let g = theme.pointer("/tokens/gradient/panel_glow_bg");
    let s = theme.pointer("/shared/color/panel_glow_bg");
    println!("gradient={g:?}");
    println!("shared={s:?}");
    println!(
        "digest={}",
        mei_lang_kernel::ops_themes_revision_digest(&cfg)
    );
}
