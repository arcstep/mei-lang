use std::path::PathBuf;

use mei_host_graph::assemble_scope_from_registry;
use mei_lang_datasets::{query_metric_dataframe, DatasetQueryOptions};

fn ws_demo_v2() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../workspaces/ws-demo-v2")
        .canonicalize()
        .expect("ws-demo-v2")
}

fn mini_data_app_root() -> PathBuf {
    ws_demo_v2().join("apps/mini-data")
}

fn first_column_name(result: &mei_lang_datasets::DatasetQueryResult) -> String {
    result
        .columns
        .first()
        .cloned()
        .or_else(|| {
            result
                .rows
                .first()
                .and_then(|row| row.as_object())
                .and_then(|map| map.keys().next())
                .cloned()
        })
        .unwrap_or_default()
}

#[test]
fn mini_data_home_scalar_rowset_respects_dataset_across_pages() {
    let workspace = ws_demo_v2();
    let app_root = mini_data_app_root();
    let outcome = assemble_scope_from_registry(workspace.as_path(), "mini-data", "home")
        .expect("assemble")
        .expect("home outcome");
    let compiled = &outcome.compiled;
    let target = "src/scene/home.mei";
    let cases = [
        (
            "supervision_matters",
            "supervision_items_count::__scalar_rowset__",
            "监督事项",
        ),
        (
            "warning_models",
            "supervision_models_count::__scalar_rowset__",
            "预警模型",
        ),
        (
            "warning_list",
            "warnings_count::__scalar_rowset__",
            "预警数量",
        ),
    ];
    for (dataset_id, metric_id, label) in cases {
        let page1 = query_metric_dataframe(
            compiled,
            app_root.as_path(),
            dataset_id,
            metric_id,
            Some("home"),
            Some(target),
            "mini-data-drilldown-rowset-test",
            DatasetQueryOptions {
                page: 1,
                page_size: 8,
                collect_all: false,
                ..DatasetQueryOptions::default()
            },
            None,
            Vec::new(),
        )
        .unwrap_or_else(|error| panic!("{label} page1 query failed: {error}"));
        let page2 = query_metric_dataframe(
            compiled,
            app_root.as_path(),
            dataset_id,
            metric_id,
            Some("home"),
            Some(target),
            "mini-data-drilldown-rowset-test",
            DatasetQueryOptions {
                page: 2,
                page_size: 8,
                collect_all: false,
                ..DatasetQueryOptions::default()
            },
            None,
            Vec::new(),
        )
        .unwrap_or_else(|error| panic!("{label} page2 query failed: {error}"));
        assert!(!page1.rows.is_empty(), "{label} page1 should return rows");
        assert!(!page2.rows.is_empty(), "{label} page2 should return rows");
        let col1 = first_column_name(&page1);
        let col2 = first_column_name(&page2);
        assert_eq!(
            col1, col2,
            "{label} page1/page2 should share schema, got {col1} vs {col2}"
        );
        assert_eq!(
            page1.total, page2.total,
            "{label} page1/page2 total should match"
        );
        match dataset_id {
            "supervision_matters" => {
                assert!(
                    ["存在的问题", "序号", "监督事项", "监督类别"]
                        .iter()
                        .any(|needle| col1.contains(needle)),
                    "supervision_matters schema mismatch, first column={col1}"
                );
            }
            "warning_models" => {
                assert!(
                    ["模型", "序号", "序号前缀", "政策文件", "监督模型"]
                        .iter()
                        .any(|needle| col1.contains(needle)),
                    "warning_models schema mismatch, first column={col1}"
                );
            }
            "warning_list" => {
                assert!(
                    ["EMPTY", "预警", "主责", "分办"]
                        .iter()
                        .any(|needle| col1.contains(needle)),
                    "warning_list schema mismatch, first column={col1}"
                );
            }
            _ => {}
        }
    }
}
