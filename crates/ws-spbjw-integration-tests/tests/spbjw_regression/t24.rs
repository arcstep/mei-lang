use serde_json::Value;
use ws_spbjw_integration_tests::{
    compile_app_from_root_with_options, evaluate_runtime_metric_defs, source_root, zhifa_app_root,
    CompileOptions,
};

#[test]
fn spbjw_shell_and_scene_theme_injection_use_separate_css_var_tracks() {
    use mei_lang_app::{page_body_theme_style, scene_viewport_theme_style};
    use mei_lang_kernel::load_workspace_config;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let workspace = load_workspace_config(&source_root);
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile spbjw home preview");
    let body_style = page_body_theme_style(&workspace, Some(&compiled), None);
    assert!(
        body_style.contains("--mei-shell-color-"),
        "workspace shell theme should inject --mei-shell-color-* on body"
    );
    assert!(
        body_style.contains("--mei-color-"),
        "page body should inject scene vars for overlays"
    );
    let scene_style = scene_viewport_theme_style(&compiled, None);
    assert!(
        scene_style.contains("--mei-color-"),
        "viewport scene theme should inject --mei-color-*"
    );
    assert!(
        !scene_style.contains("--mei-shell-color-"),
        "viewport must not inject shell color vars"
    );
}

#[test]
fn spbjw_live_ops_theme_overlay_overrides_compile_snapshot_without_recompile() {
    use mei_lang_app::scene_viewport_theme_style;
    use mei_lang_kernel::{
        load_mei_config_for_app, mei_config_compile_revision_digest, CompileOptions,
    };
    use serde_json::json;

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let compile_options = CompileOptions {
        scene: None,
        preview_target: Some("scenes/home.mei".to_string()),
    };
    let config_on_disk = load_mei_config_for_app(&app_root, Some(source_root.as_path()));
    let digest_before = mei_config_compile_revision_digest(&config_on_disk);
    let compiled = compile_app_from_root_with_options(&source_root, &app_root, compile_options)
        .expect("compile spbjw home preview");

    let mut live = config_on_disk;
    let cockpit = live
        .ops
        .themes
        .get_mut("cockpit")
        .expect("zhifa cockpit theme");
    if let Some(tokens) = cockpit
        .pointer_mut("/tokens/color")
        .and_then(Value::as_object_mut)
    {
        tokens.insert("panel_title".to_string(), json!("#aabbcc"));
    } else {
        cockpit["tokens"]["color"]["panel_title"] = json!("#aabbcc");
    }
    let digest_after = mei_config_compile_revision_digest(&live);
    assert_eq!(
        digest_before, digest_after,
        "theme-only ops change must not alter compile revision digest"
    );

    let from_disk = scene_viewport_theme_style(&compiled, None);
    let from_live = scene_viewport_theme_style(&compiled, Some(&live));
    assert!(
        from_live.contains("--mei-color-panel-title:#aabbcc"),
        "live overlay should inject mutated panel_title: {from_live}"
    );
    assert_ne!(
        from_disk, from_live,
        "in-memory live overlay should differ from on-disk auto-load"
    );
}

#[test]
fn spbjw_disk_config_font_28px_in_viewport_style() {
    use mei_lang_app::scene_viewport_theme_style;
    use mei_lang_kernel::{load_mei_config_for_app, CompileOptions};
    use ws_spbjw_integration_tests::{
        compile_app_from_root_with_options, source_root, zhifa_app_root,
    };

    let source_root = source_root();
    let app_root = zhifa_app_root();
    let config = load_mei_config_for_app(&app_root, Some(source_root.as_path()));
    let compiled = compile_app_from_root_with_options(
        &source_root,
        &app_root,
        CompileOptions {
            scene: None,
            preview_target: Some("scenes/home.mei".to_string()),
        },
    )
    .expect("compile");
    let with_live = scene_viewport_theme_style(&compiled, Some(&config));
    let auto_load = scene_viewport_theme_style(&compiled, None);
    assert!(
        with_live.contains("--mei-font-2:28px"),
        "live: {}",
        &with_live[..with_live.len().min(800)]
    );
    assert!(
        auto_load.contains("--mei-font-2:28px"),
        "auto app_root={}: {}",
        compiled.app_root,
        &auto_load[..auto_load.len().min(800)]
    );
}
