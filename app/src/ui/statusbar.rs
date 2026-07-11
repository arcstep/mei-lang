use leptos::prelude::*;
use mei_lang_kernel::is_stock_catalog_app;

use super::manage_routing::OPS_CONFIG_TARGET;

pub(crate) fn statusbar_view(
    app_path: &str,
    route_mode: &'static str,
    current_target: &str,
    upload_root: Option<&str>,
) -> AnyView {
    let show_visit_history = matches!(route_mode, "app" | "access");
    let left_path = if show_visit_history {
        None
    } else {
        statusbar_left_path(app_path, route_mode, current_target, upload_root)
    };
    view! {
        <footer class="statusbar statusbar-shell chrome-inset chrome-safe-x sticky bottom-0 z-10 py-1.5 backdrop-blur-md">
            <div class="statusbar-layout min-w-0 mei-font-1">
                <div class="statusbar-track statusbar-track-left min-w-0">
                    {if show_visit_history {
                        view! {
                            <button
                                type="button"
                                id="mei-visit-history-trigger"
                                class="status-chip status-chip-visit-history"
                                data-tone="neutral"
                                title="最近访问与加载耗时"
                            >
                                "访问历史"
                            </button>
                        }
                        .into_any()
                    } else if let Some(path) = left_path.as_deref() {
                        view! {
                            <span
                                class="status-chip status-chip-file status-chip-path max-w-[40vw]"
                                title=path
                            >
                                {path}
                            </span>
                        }
                        .into_any()
                    } else {
                        view! { <></> }.into_any()
                    }}
                </div>
                <div class="statusbar-track statusbar-track-center min-w-0">
                    <span
                        class="status-chip status-chip-compliance max-w-[min(52vw,560px)]"
                        id="mei-status-compliance"
                        data-tone="neutral"
                        hidden
                    ></span>
                </div>
                <span
                    class="status-chip status-chip-host statusbar-right-anchor"
                    id="mei-status-host-version"
                    data-tone="neutral"
                    title="__MEI_HOST_VERSION_TITLE__"
                >
                    "__MEI_HOST_VERSION_LABEL__"
                </span>
            </div>
        </footer>
    }
    .into_any()
}

fn workspace_relative_path(app_path: &str, target: &str) -> String {
    let app = app_path.trim().trim_matches('/');
    let target = target.trim().trim_start_matches("./");
    if app.is_empty() {
        target.to_string()
    } else if target.is_empty() {
        app.to_string()
    } else {
        format!("{app}/{target}")
    }
}

fn collapse_parent_path_segments(raw: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for part in raw
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
    {
        if part == ".." {
            stack.pop();
        } else {
            stack.push(part);
        }
    }
    stack.join("/")
}

fn build_statusbar_display_path(app_path: &str, target: &str) -> String {
    let joined = workspace_relative_path(app_path, target);
    let collapsed = collapse_parent_path_segments(joined.as_str());
    if is_stock_catalog_app(app_path) {
        if let Some(idx) = collapsed.find("stock/") {
            return collapsed[idx..].to_string();
        }
    }
    collapsed
}

fn statusbar_left_path(
    app_path: &str,
    route_mode: &str,
    current_target: &str,
    upload_root: Option<&str>,
) -> Option<String> {
    match route_mode {
        "workspace" => {
            let path = current_target.trim();
            if path.is_empty() {
                None
            } else {
                Some(path.to_string())
            }
        }
        "build" | "manage" | "layout" | "prototype" => {
            let target = current_target.trim();
            if target.is_empty() {
                None
            } else {
                Some(build_statusbar_display_path(app_path, target))
            }
        }
        "config" => Some(workspace_relative_path(app_path, OPS_CONFIG_TARGET)),
        "upload" => {
            let root = upload_root.unwrap_or("upload").trim();
            let selected = current_target.trim();
            let suffix = if selected.is_empty() || selected == root {
                root.to_string()
            } else if selected.starts_with(&format!("{root}/")) {
                selected.to_string()
            } else {
                format!("{root}/{selected}")
            };
            Some(workspace_relative_path(app_path, &suffix))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_left_path_joins_app_and_target() {
        assert_eq!(
            statusbar_left_path("zhifa", "build", "scenes/home.mei", None).as_deref(),
            Some("zhifa/scenes/home.mei")
        );
    }

    #[test]
    fn config_left_path_points_at_mei_config() {
        assert_eq!(
            statusbar_left_path("zhifa", "config", "", None).as_deref(),
            Some("zhifa/.mei-config.json")
        );
    }

    #[test]
    fn build_stock_catalog_path_collapses_parent_segments() {
        assert_eq!(
            statusbar_left_path(
                "_stock-catalog",
                "build",
                "../../stock/components/chart/echarts/previews/chart.trend.mei",
                None,
            )
            .as_deref(),
            Some("stock/components/chart/echarts/previews/chart.trend.mei")
        );
    }

    #[test]
    fn upload_left_path_prefixes_upload_root() {
        assert_eq!(
            statusbar_left_path("zhifa", "upload", "1.xlsx", Some("upload")).as_deref(),
            Some("zhifa/upload/1.xlsx")
        );
        assert_eq!(
            statusbar_left_path("zhifa", "upload", "", Some("upload")).as_deref(),
            Some("zhifa/upload")
        );
    }
}
