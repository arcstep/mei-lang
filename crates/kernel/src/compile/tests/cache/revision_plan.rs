use super::helpers::*;

#[test]
fn compile_revision_plan_watches_mei_config_but_theme_only_change_does_not_invalidate() {
    let root = std::env::temp_dir().join(format!("mei-revision-config-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("scenes")).unwrap();
    fs::write(
        root.join("main.mei"),
        r#"
app(
    id = "config-revision-test",
    default_scene = "home",
    scene = scene_ref(scene_file = "scenes/home.mei")
)
"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/home.mei"),
        r#"
scene(id = "home", theme = theme_ref("cockpit"))
world()
frame()
"#,
    )
    .unwrap();
    fs::write(
        root.join(".mei-config.json"),
        r#"{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" },
  "ops": {
    "themes": {
      "cockpit": {
        "font": { "2": "14px" }
      }
    }
  }
}"#,
    )
    .unwrap();
    let options = CompileOptions {
        scene: Some("home".to_string()),
        preview_target: None,
    };
    let first = compile_revision_plan_from_root_with_options(&root, &root, &options)
        .expect("first revision plan");
    assert!(
        first
            .watched_files
            .iter()
            .any(|item| item.rel_path == ".mei-config.json"),
        "revision plan should watch .mei-config.json"
    );
    let first_token = first.token;
    std::thread::sleep(std::time::Duration::from_millis(5));
    fs::write(
        root.join(".mei-config.json"),
        r#"{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" },
  "ops": {
    "themes": {
      "cockpit": {
        "font": { "2": "16px" }
      }
    }
  }
}"#,
    )
    .unwrap();
    let second_token = compile_revision_token_from_root_with_options(&root, &root, &options)
        .expect("second token");
    assert_eq!(
        first_token, second_token,
        "theme-only change in ops.themes should not invalidate compile revision"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_revision_plan_invalidates_on_mei_config_params_change() {
    let root = std::env::temp_dir().join(format!(
        "mei-revision-config-entry-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("scenes")).unwrap();
    fs::write(
        root.join("main.mei"),
        r#"
app(
    id = "config-revision-test",
    default_scene = "home",
    scene = scene_ref(scene_file = "scenes/home.mei")
)
"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/home.mei"),
        r#"
scene(id = "home", theme = theme_ref("cockpit"))
world()
frame()
"#,
    )
    .unwrap();
    fs::write(
        root.join(".mei-config.json"),
        r#"{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" },
  "ops": { "themes": { "cockpit": { "font": { "2": "14px" } } } }
}"#,
    )
    .unwrap();
    let options = CompileOptions {
        scene: Some("home".to_string()),
        preview_target: None,
    };
    let first_token =
        compile_revision_token_from_root_with_options(&root, &root, &options).expect("first");
    fs::write(
        root.join(".mei-config.json"),
        r#"{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" },
  "ops": {
    "themes": { "cockpit": { "font": { "2": "14px" } } },
    "params": { "accent": "blue" }
  }
}"#,
    )
    .unwrap();
    let second_token =
        compile_revision_token_from_root_with_options(&root, &root, &options).expect("second");
    assert_ne!(
        first_token, second_token,
        "ops.params change should invalidate compile revision"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn compile_revision_plan_watches_ops_source_files_and_invalidates_on_data_change() {
    let root = std::env::temp_dir().join(format!("mei-revision-source-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("upload")).unwrap();
    fs::create_dir_all(root.join("scenes")).unwrap();
    fs::write(
        root.join("main.mei"),
        r#"
app(
    id = "source-revision-test",
    default_scene = "home",
    scene = scene_ref(scene_file = "scenes/home.mei")
)
"#,
    )
    .unwrap();
    fs::write(
        root.join("scenes/home.mei"),
        r#"
scene(id = "home", theme = theme_ref("cockpit"))
world()
frame()
"#,
    )
    .unwrap();
    fs::write(root.join("upload/data.csv"), "a,1\n").unwrap();
    fs::write(
        root.join(".mei-config.json"),
        r#"{
  "schemaVersion": 1,
  "entry": { "main": "main.mei" },
  "ops": {
    "sources": {
      "demo": {
        "kind": "csv",
        "path": "upload/data.csv"
      }
    }
  }
}"#,
    )
    .unwrap();
    let options = CompileOptions {
        scene: Some("home".to_string()),
        preview_target: None,
    };
    let first = compile_revision_plan_from_root_with_options(&root, &root, &options)
        .expect("first revision plan");
    assert!(
        first
            .watched_files
            .iter()
            .any(|item| item.rel_path == "upload/data.csv"),
        "revision plan should watch ops.sources data files, got {:?}",
        first
            .watched_files
            .iter()
            .map(|item| item.rel_path.as_str())
            .collect::<Vec<_>>()
    );
    let first_token = first.token;
    std::thread::sleep(std::time::Duration::from_millis(5));
    fs::write(root.join("upload/data.csv"), "a,2\nb,3\n").unwrap();
    let second_token = compile_revision_token_from_root_with_options(&root, &root, &options)
        .expect("second token");
    assert_ne!(
        first_token, second_token,
        "ops.sources data file change should invalidate compile revision"
    );
    let _ = fs::remove_dir_all(&root);
}

