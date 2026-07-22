fn main() {
    let app = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .expect("usage: dump_theme <app-root>");
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
