use std::time::Instant;

use mei_lang_kernel::{CompiledApp, Diagnostic, Severity};
use serde_json::json;

pub(crate) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

pub(crate) fn is_script_target(path: &str) -> bool {
    path.ends_with(".mei")
}

/// 管理端整页流水线：发现应用 → 编译（含缓存）→ 读源码 → SSR（多遍）→ 总耗时；
/// 另附产物规模与「运行时数据查询」待 iframe 上报说明。消息体为 **JSON**，供客户端齐整渲染。
///
/// SSR 说明：`ssr_baseline_ms` 为「尚无本诊断条目」时的首遍 SSR；`ssr_publish_ms` 为插入本条目后
/// 一遍 SSR（用于校准耗时，此时 JSON 内 publish 可能仍为占位）；`ssr_final_emit_ms` 为写入 baseline/publish/final
/// 三段时间后的再渲染。若提供 `ssr_response_ms`，会追加「响应遍」阶段（其 ms 为**上一轮** SSR 的耗时，
/// 再经一轮 SSR 产出与 HTTP 响应一致的 HTML）。若提供 `ssr_serve_ms`，表示再一轮 SSR（JSON 已含
/// `ssr_response` 行之后）的耗时。若提供 `ssr_emit_ms`，表示再一遍（将含 `ssr_serve` 与对齐后的
/// `server_total` 写入 JSON 后的渲染，**通常即下发 HTML 的那遍**）。`total_ms` 建议取 **末遍 SSR
/// 完成之后** 的 handler 墙钟。
pub(crate) fn push_manage_page_pipeline_diag(
    compiled: &mut CompiledApp,
    app_id: &str,
    target: &str,
    discover_ms: u64,
    compile_ms: u64,
    compile_cache_hit: bool,
    compile_cache_lookup_ms: u64,
    source_read_ms: u64,
    ssr_baseline_ms: u64,
    ssr_publish_ms: u64,
    ssr_final_emit_ms: u64,
    ssr_response_ms: Option<u64>,
    ssr_serve_ms: Option<u64>,
    ssr_emit_ms: Option<u64>,
    total_ms: u64,
) {
    let resource_count = compiled.resources.len();
    let dataset_resources = compiled
        .resources
        .iter()
        .filter(|r| r.dataset.is_some())
        .count();
    let mut stages = vec![
        json!({
            "id": "discover_apps",
            "label": "发现应用列表",
            "status": "ok",
            "ms": discover_ms,
            "detail": {}
        }),
        json!({
            "id": "compile_app",
            "label": "编译（入口 / scene / 资源物化）",
            "status": "ok",
            "ms": compile_ms,
            "detail": {
                "compile_cache_hit": compile_cache_hit,
                "compile_cache_lookup_ms": compile_cache_lookup_ms,
                "resource_count": resource_count,
                "dataset_resources": dataset_resources,
                "hint": "widget 预览应 selective catalog（dataset_resources 远小于 21）且不编译 home；需重启 mei-lang-server 后生效"
            }
        }),
        json!({
            "id": "source_read",
            "label": "读取目标脚本（源码面板）",
            "status": "ok",
            "ms": source_read_ms,
            "detail": {}
        }),
        json!({
            "id": "ssr_baseline",
            "label": "SSR 首遍（尚无 manage_page_pipeline 诊断）",
            "status": "ok",
            "ms": ssr_baseline_ms,
            "detail": {
                "hint": "用于对比「插入流水线 JSON 前后」的渲染成本；该遍 HTML 不对外下发"
            }
        }),
        json!({
            "id": "ssr_publish",
            "label": "SSR 二遍（已含诊断条目；用于测量含面板树时的渲染）",
            "status": "ok",
            "ms": ssr_publish_ms,
            "detail": {
                "hint": "此时 JSON 内数值可能仍为占位，仅用于耗时校准"
            }
        }),
        json!({
            "id": "ssr_final_emit",
            "label": "SSR 三遍（写入 baseline/publish/final 三段时间后）",
            "status": "ok",
            "ms": ssr_final_emit_ms,
            "detail": {
                "hint": "该遍 HTML 仍不含「响应遍」耗时字段；若存在 ssr_response 阶段则最终响应为再下一遍 SSR"
            }
        }),
    ];
    if let Some(ms) = ssr_response_ms {
        stages.push(json!({
            "id": "ssr_response",
            "label": "SSR 响应遍（将上一轮 SSR 耗时写入 JSON 后的再渲染）",
            "status": "ok",
            "ms": ms,
            "detail": {
                "hint": "本阶段 ms 为上一轮（含完整三段时间）SSR 的耗时；当前 HTTP 响应 HTML 为紧随其后的再渲染结果"
            }
        }));
    }
    if let Some(ms) = ssr_serve_ms {
        stages.push(json!({
            "id": "ssr_serve",
            "label": "SSR 定稿遍（JSON 已含 ssr_response 行；本遍结束后再对齐 server_total）",
            "status": "ok",
            "ms": ms,
            "detail": {
                "hint": "此前若将 server_total 写在定稿遍之前，会少计约一整遍 SSR"
            }
        }));
    }
    if let Some(ms) = ssr_emit_ms {
        stages.push(json!({
            "id": "ssr_emit",
            "label": "SSR 末遍（与当前 HTTP 响应 HTML 对齐）",
            "status": "ok",
            "ms": ms,
            "detail": {
                "hint": "将含 ssr_serve 与更新后的 server_total 的 JSON 渲染进页面"
            }
        }));
    }
    stages.push(json!({
        "id": "server_total",
        "label": "服务端 manage 页处理器墙钟（至写入本 JSON 前一刻）",
        "status": "ok",
        "ms": total_ms,
        "detail": {
            "scope": "mei-lang-server app_page handler + 多遍 SSR",
            "excludes": [
                "浏览器网络排队、TLS、下载与解析 JS/CSS/字体",
                "首屏后 hydration、预览 iframe 内 fetch（见 runtime_perf）",
                "本请求 compile_ms=0 仅表示命中编译缓存；体感「前几秒」常来自浏览器侧或其它请求",
                "若存在 ssr_emit 行：本 server_total.ms 仍不含「为输出最终 HTML」在写入本 JSON 之后再跑的那一遍 SSR（与页面 shell 上 total_ms=__TOTAL_MS__ 的差额约等于该遍）",
                "管理页 diagnostics/source 标签时「应用预览」区为 HTML hidden，但 SSR 仍会输出预览 DOM；此前 dataset 组件在 connectedCallback 即拉 /api/datasets/query 或 ECharts CDN，主线程阻塞不会反映在本 JSON。已在 workspaces/_components 对 table/summary-cards/chart 做「可见后再初始化」（并监听 mei:manage-tab-change）",
                "从 app_page 入口到 Html 字符串就绪的完整墙钟见 request_timing 与响应头 X-Mei-Handler-Html-Ready-Ms（与末遍 SSR 耗时 X-Mei-Ssr-Http-Response-Body-Ms）；仍不含 Axum 将字节写入 TCP 与浏览器解析"
            ]
        }
    }));
    let payload = json!({
        "schema_version": 1,
        "kind": "manage_page_pipeline",
        "app_id": app_id,
        "target": target,
        "summary_status": "ok",
        "stages": stages,
        "artifact_stats": {
            "resources": resource_count,
            "dataset_resources": dataset_resources
        },
        "runtime_pending": [
            {
                "id": "dataset_query",
                "label": "数据绑定 / 外部数据源动态查询",
                "hint": "由预览区 mei-dataset-table / dataset.summary-cards 上报；见下方 runtime_perf（含 client_ttfb_ms、server_handler_total_ms、client_outside_server_ms）"
            }
        ],
        "request_timing": {
            "hint": "精确墙钟见 HTTP 响应头 X-Mei-Handler-Html-Ready-Ms / X-Mei-Ssr-Http-Response-Body-Ms，以及 head 中 meta mei-handler-html-ready-ms、body data-mei-handler-html-ready-ms（由 fill_manage_wall_clock_placeholders 在末遍 SSR 之后写入）",
            "handler_html_ready_ms": "__MEI_HANDLER_HTML_READY_MS__",
            "ssr_http_response_body_ms": "__MEI_SSR_HTTP_BODY_MS__"
        }
    });
    let pretty = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string());
    compiled.diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "manage_page_pipeline".to_string(),
        message: pretty,
        source_path: Some(target.to_string()),
    });
}

/// SSR 页面体积观测：用于诊断 `data-props` 重复膨胀导致的慢首屏。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PageHtmlPayloadStats {
    pub html_bytes: usize,
    pub data_props_count: usize,
    pub data_props_bytes: usize,
    pub data_props_max_bytes: usize,
    pub scene_drilldown_context_bytes: usize,
}

const DATA_PROPS_ATTR: &str = "data-props=\"";
const DATA_PROPS_ATTR_LEGACY: &str = "data-props='";
const SCENE_DRILLDOWN_CONTEXT_ID: &str = "id=\"mei-scene-drilldown-context\"";

pub(crate) fn measure_page_html_payload(html: &str) -> PageHtmlPayloadStats {
    let html_bytes = html.len();
    let mut data_props_count = 0usize;
    let mut data_props_bytes = 0usize;
    let mut data_props_max_bytes = 0usize;

    let mut search_from = 0usize;
    while search_from < html.len() {
        let tail = &html[search_from..];
        let (attr, payload_start) = if let Some(rel) = tail.find(DATA_PROPS_ATTR) {
            (DATA_PROPS_ATTR, search_from + rel + DATA_PROPS_ATTR.len())
        } else if let Some(rel) = tail.find(DATA_PROPS_ATTR_LEGACY) {
            (
                DATA_PROPS_ATTR_LEGACY,
                search_from + rel + DATA_PROPS_ATTR_LEGACY.len(),
            )
        } else {
            break;
        };
        let payload = &html[payload_start..];
        let end_rel = if attr == DATA_PROPS_ATTR {
            payload.find('"')
        } else {
            payload.find('\'')
        };
        if let Some(end_rel) = end_rel {
            data_props_count += 1;
            data_props_bytes += end_rel;
            data_props_max_bytes = data_props_max_bytes.max(end_rel);
            search_from = payload_start + end_rel + 1;
        } else {
            break;
        }
    }

    PageHtmlPayloadStats {
        html_bytes,
        data_props_count,
        data_props_bytes,
        data_props_max_bytes,
        scene_drilldown_context_bytes: scene_drilldown_context_json_bytes(html),
    }
}

fn scene_drilldown_context_json_bytes(html: &str) -> usize {
    let Some((_, after_id)) = html.split_once(SCENE_DRILLDOWN_CONTEXT_ID) else {
        return 0;
    };
    let Some((_, after_open)) = after_id.split_once('>') else {
        return 0;
    };
    let Some((json, _)) = after_open.split_once("</script>") else {
        return 0;
    };
    json.len()
}

pub(crate) fn fill_perf_placeholders(mut html: String, render_ms: u64, total_ms: u64) -> String {
    html = html.replace(
        "render_ms=__RENDER_MS__",
        format!("render_ms={render_ms}ms").as_str(),
    );
    html = html.replace(
        "total_ms=__TOTAL_MS__",
        format!("total_ms={total_ms}ms").as_str(),
    );
    html
}

/// 将管理页「请求结束」墙钟写入 HTML（meta / body data-* / manage_page_pipeline JSON 内占位符）。
/// 占位符与 `push_manage_page_pipeline_diag` 产出的 `request_timing` 字段一致。
pub(crate) fn fill_manage_wall_clock_placeholders(
    mut html: String,
    ssr_http_response_body_ms: u64,
    handler_html_ready_ms: u64,
) -> String {
    let body = ssr_http_response_body_ms.to_string();
    let ready = handler_html_ready_ms.to_string();
    html = html.replace("__MEI_SSR_HTTP_BODY_MS__", body.as_str());
    html = html.replace("__MEI_HANDLER_HTML_READY_MS__", ready.as_str());
    html
}

/// 首屏加载进度条：将编译与 HTML 体积观测写入 body data-* 占位符。
pub(crate) fn fill_page_load_observability_placeholders(
    mut html: String,
    compile_ms: u64,
    compile_cache_hit: bool,
    html_bytes: usize,
    data_props_bytes: usize,
    data_props_count: usize,
) -> String {
    html = html.replace("__MEI_COMPILE_MS__", &compile_ms.to_string());
    html = html.replace(
        "__MEI_COMPILE_CACHE_HIT__",
        if compile_cache_hit { "1" } else { "0" },
    );
    html = html.replace("__MEI_HTML_BYTES__", &html_bytes.to_string());
    html = html.replace("__MEI_DATA_PROPS_BYTES__", &data_props_bytes.to_string());
    html = html.replace("__MEI_DATA_PROPS_COUNT__", &data_props_count.to_string());
    html
}

/// 将 Martin 瓦片服务地址写入 HTML（`meta[name=mei-tiles-*]`），供 `map.maplibre` 读取。
pub(crate) fn fill_gis_tiles_placeholders(
    mut html: String,
    cfg: &crate::gis_config::GisTilesConfig,
) -> String {
    html = html.replace("__MEI_TILES_BASE_URL__", cfg.base_url.as_str());
    html = html.replace("__MEI_TILES_JSON_PATH__", cfg.json_path.as_str());
    html
}

pub(crate) fn fill_host_build_placeholders(mut html: String) -> String {
    let version_label = crate::build_info::version_label_with_target();
    html = html.replace("__MEI_HOST_VERSION__", crate::build_info::BUILD_VERSION);
    html = html.replace("__MEI_HOST_VERSION_LABEL__", version_label.as_str());
    html = html.replace("__MEI_HOST_VERSION_TITLE__", version_label.as_str());
    html
}

pub(crate) fn fill_host_compliance_placeholders(
    mut html: String,
    source_root: &std::path::Path,
) -> String {
    let workspace = mei_lang_kernel::load_workspace_config(source_root);
    html = html.replace(
        "__MEI_HOST_ICP_RECORD__",
        workspace.compliance.icp_record_trimmed().unwrap_or(""),
    );
    html = html.replace(
        "__MEI_HOST_PSB_RECORD__",
        workspace.compliance.psb_record_trimmed().unwrap_or(""),
    );
    html = html.replace(
        "__MEI_HOST_COPYRIGHT__",
        workspace.compliance.copyright_trimmed().unwrap_or(""),
    );
    html = html.replace(
        "__MEI_WORKSPACE_LABEL__",
        workspace
            .workspace
            .label
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(""),
    );
    html
}

pub(crate) fn fill_page_shell_placeholders(
    html: String,
    cfg: &crate::gis_config::GisTilesConfig,
    source_root: &std::path::Path,
) -> String {
    fill_host_compliance_placeholders(
        fill_host_build_placeholders(fill_gis_tiles_placeholders(html, cfg)),
        source_root,
    )
}

#[cfg(test)]
mod tests {
    use super::{measure_page_html_payload, PageHtmlPayloadStats};

    #[test]
    fn measure_page_html_payload_counts_data_props_and_scene_context() {
        let html = concat!(
            "<mei-text data-props=\"{&quot;a&quot;:1}\"></mei-text>",
            "<mei-text data-props='{\"b\":22}'></mei-text>",
            "<script id=\"mei-scene-drilldown-context\" type=\"application/json\">",
            "{\"scene_bindings_by_id\":{}}",
            "</script>",
        );
        let stats = measure_page_html_payload(html);
        assert_eq!(
            stats,
            PageHtmlPayloadStats {
                html_bytes: html.len(),
                data_props_count: 2,
                data_props_bytes: 7 + 8,
                data_props_max_bytes: 8,
                scene_drilldown_context_bytes: "{\"scene_bindings_by_id\":{}}".len(),
            }
        );
    }
}
