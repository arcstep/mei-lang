use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::super::compile_status::{
    compile_diagnostics_for_mode, compile_diagnostics_other_file_count, is_manage_pipeline_diag,
    normalize_diagnostic_source, severity_counts, DiagnosticsFilterMode,
};
use super::super::manage_routing::{manage_tab_href, ManageViewTab};
use super::html_escape::escape_html_attr;

fn compile_diag_card(diag: &mei_lang_kernel::Diagnostic, compiled: &CompiledApp) -> AnyView {
    let class = match diag.severity {
        mei_lang_kernel::Severity::Error => {
            "diag mei-compile-diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-red-900/25 border-red-400/30"
        }
        mei_lang_kernel::Severity::Warning => {
            "diag mei-compile-diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-amber-900/25 border-amber-300/35"
        }
        mei_lang_kernel::Severity::Info => {
            "diag mei-compile-diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-blue-900/25 border-blue-300/35"
        }
    };
    let source_label = normalize_diagnostic_source(&compiled.app_root, diag.source_path.as_deref())
        .unwrap_or_else(|| "（未标注来源）".to_string());
    let source_attr = escape_html_attr(&source_label);
    view! {
        <div
            class=class
            data-mei-compile-diag="true"
            attr:data-diag-code=diag.code.clone()
            attr:data-diag-source=source_attr
        >
            <strong class="text-xs font-semibold text-slate-50">{diag.code.clone()}</strong>
            <span class="text-xs leading-5 text-slate-200">{diag.message.clone()}</span>
            <span class="text-[10px] font-mono text-slate-500">{format!("来源：{source_label}")}</span>
        </div>
    }
    .into_any()
}

fn manage_page_pipeline_diag_view(diag: &mei_lang_kernel::Diagnostic) -> AnyView {
    let base_class =
        "diag mt-2 grid gap-2 rounded-xl border px-3 py-2 bg-blue-900/25 border-blue-300/35";
    match serde_json::from_str::<serde_json::Value>(&diag.message) {
        Ok(v) if v.get("kind").and_then(|k| k.as_str()) == Some("manage_page_pipeline") => {
            let summary = v
                .get("summary_status")
                .and_then(|s| s.as_str())
                .unwrap_or("—")
                .to_string();
            let app_id = v
                .get("app_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let target = v
                .get("target")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let stage_rows: Vec<(String, String, String, String)> = v
                .get("stages")
                .and_then(|s| s.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|st| {
                            let id = st.get("id")?.as_str()?.to_string();
                            let label = st
                                .get("label")
                                .and_then(|l| l.as_str())
                                .unwrap_or("")
                                .to_string();
                            let status = st
                                .get("status")
                                .and_then(|l| l.as_str())
                                .unwrap_or("")
                                .to_string();
                            let ms_str = match st.get("ms") {
                                Some(m) if m.is_null() => "—".to_string(),
                                Some(m) => m
                                    .as_u64()
                                    .map(|n| format!("{n} ms"))
                                    .or_else(|| m.as_f64().map(|n| format!("{n} ms")))
                                    .unwrap_or_else(|| m.to_string()),
                                None => "—".to_string(),
                            };
                            Some((id, label, status, ms_str))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let stages_table = stage_rows
                .into_iter()
                .map(|(id, label, status, ms_str)| {
                    view! {
                        <tr class="border-b border-slate-800/80 text-[11px]">
                            <td class="whitespace-nowrap px-2 py-1.5 font-mono text-slate-400">{id}</td>
                            <td class="px-2 py-1.5 text-slate-200">{label}</td>
                            <td class="whitespace-nowrap px-2 py-1.5 text-slate-300">{status}</td>
                            <td class="whitespace-nowrap px-2 py-1.5 text-right font-mono text-slate-200">{ms_str}</td>
                        </tr>
                    }
                })
                .collect_view();
            let resources = v
                .get("artifact_stats")
                .and_then(|a| a.get("resources"))
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            let datasets = v
                .get("artifact_stats")
                .and_then(|a| a.get("dataset_resources"))
                .and_then(|x| x.as_u64())
                .unwrap_or(0);
            let pending: Vec<String> = v
                .get("runtime_pending")
                .and_then(|p| p.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            let id = item.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let label = item.get("label").and_then(|l| l.as_str()).unwrap_or("");
                            let hint = item.get("hint").and_then(|h| h.as_str()).unwrap_or("");
                            if id.is_empty() && label.is_empty() {
                                return None;
                            }
                            Some(format!("{} — {} · {}", id, label, hint))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let pending_view = if pending.is_empty() {
                view! { <></> }.into_any()
            } else {
                view! {
                    <ul class="m-0 list-disc space-y-1 pl-4 text-[10px] leading-4 text-slate-400">
                        {pending
                            .into_iter()
                            .map(|line| {
                                view! { <li class="break-words">{line}</li> }
                            })
                            .collect_view()}
                    </ul>
                }
                .into_any()
            };
            let timing_block = v.get("request_timing").map(|rt| {
                let hint = rt
                    .get("hint")
                    .and_then(|h| h.as_str())
                    .unwrap_or("")
                    .to_string();
                let h_ms = rt
                    .get("handler_html_ready_ms")
                    .map(|x| {
                        x.as_u64()
                            .map(|n| n.to_string())
                            .or_else(|| x.as_str().map(|s| s.to_string()))
                            .unwrap_or_else(|| x.to_string())
                    })
                    .unwrap_or_else(|| "—".to_string());
                let b_ms = rt
                    .get("ssr_http_response_body_ms")
                    .map(|x| {
                        x.as_u64()
                            .map(|n| n.to_string())
                            .or_else(|| x.as_str().map(|s| s.to_string()))
                            .unwrap_or_else(|| x.to_string())
                    })
                    .unwrap_or_else(|| "—".to_string());
                view! {
                    <div class="mt-2 grid gap-1 rounded-lg border border-slate-600/40 bg-slate-950/40 p-2">
                        <span class="text-[10px] font-semibold uppercase tracking-wide text-slate-500">
                            "请求墙钟（服务端）"
                        </span>
                        <div class="grid gap-0.5 text-[11px] leading-5 text-slate-200">
                            <div>
                                <span class="text-slate-400">"handler_html_ready_ms "</span>
                                <span class="font-mono">{h_ms.clone()}</span>
                                <span class="text-slate-500">" · 亦见响应头 "</span>
                                <code class="text-slate-300">"X-Mei-Handler-Html-Ready-Ms"</code>
                            </div>
                            <div>
                                <span class="text-slate-400">"ssr_http_response_body_ms "</span>
                                <span class="font-mono">{b_ms.clone()}</span>
                                <span class="text-slate-500">" · 亦见 "</span>
                                <code class="text-slate-300">"X-Mei-Ssr-Http-Response-Body-Ms"</code>
                            </div>
                            {if hint.is_empty() {
                                view! { <></> }.into_any()
                            } else {
                                view! {
                                    <p class="m-0 text-[10px] leading-4 text-slate-500">{hint}</p>
                                }
                                .into_any()
                            }}
                        </div>
                    </div>
                }
                .into_any()
            });
            let json_compact =
                serde_json::to_string(&v).unwrap_or_else(|_| diag.message.clone());
            let json_attr = escape_html_attr(&json_compact);
            view! {
                <div class=base_class attr:data-manage-pipeline-json=json_attr>
                    <strong class="text-xs font-semibold text-slate-50">"manage_page_pipeline"</strong>
                    <div class="text-[11px] text-slate-400">
                        {format!("应用 {} · 目标 {} · {}", app_id, target, summary)}
                    </div>
                    <div class="overflow-x-auto rounded-lg border border-slate-600/40">
                        <table class="w-full border-collapse text-left">
                            <thead>
                                <tr class="bg-slate-900/90 text-[10px] uppercase tracking-wide text-slate-500">
                                    <th class="px-2 py-1.5">"id"</th>
                                    <th class="px-2 py-1.5">"环节"</th>
                                    <th class="px-2 py-1.5">"状态"</th>
                                    <th class="px-2 py-1.5 text-right">"耗时"</th>
                                </tr>
                            </thead>
                            <tbody>{stages_table}</tbody>
                        </table>
                    </div>
                    <div class="text-[10px] text-slate-500">
                        {format!("产物规模：resources={} · dataset_resources={}", resources, datasets)}
                    </div>
                    <div class="grid gap-1">
                        <span class="text-[10px] font-semibold uppercase tracking-wide text-slate-500">
                            "运行时（待 iframe 上报）"
                        </span>
                        {pending_view}
                    </div>
                    {timing_block.unwrap_or_else(|| view! { <></> }.into_any())}
                    <details class="mt-1">
                        <summary class="cursor-pointer text-[11px] text-slate-400">"原始 JSON"</summary>
                        <pre class="mt-1 max-h-48 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-slate-950/80 p-2 font-mono text-[10px] leading-4 text-slate-300">{diag.message.clone()}</pre>
                    </details>
                </div>
            }
            .into_any()
        }
        _ => view! {
            <div class=base_class>
                <strong class="text-xs font-semibold text-slate-50">"manage_page_pipeline"</strong>
                <pre class="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words font-mono text-[10px] text-slate-300">{diag.message.clone()}</pre>
            </div>
        }
        .into_any(),
    }
}

pub(super) fn diagnostics_view(
    compiled: &CompiledApp,
    app_path: &str,
    selected_target: &str,
    filter_mode: DiagnosticsFilterMode,
) -> AnyView {
    if compiled.diagnostics.is_empty() {
        return view! { <></> }.into_any();
    }
    let pipeline_views = compiled
        .diagnostics
        .iter()
        .filter(|diag| is_manage_pipeline_diag(diag))
        .map(manage_page_pipeline_diag_view)
        .collect_view();
    let compile_list = compile_diagnostics_for_mode(compiled, selected_target, filter_mode);
    let (cur_e, cur_w, cur_i) = severity_counts(&compile_diagnostics_for_mode(
        compiled,
        selected_target,
        DiagnosticsFilterMode::CurrentFile,
    ));
    let (all_e, all_w, all_i) = severity_counts(&compile_diagnostics_for_mode(
        compiled,
        selected_target,
        DiagnosticsFilterMode::All,
    ));
    let other_count = compile_diagnostics_other_file_count(compiled, selected_target);
    let compile_cards = compile_list
        .iter()
        .map(|diag| compile_diag_card(diag, compiled))
        .collect_view();
    let empty_compile_hint = if compile_list.is_empty() {
        view! {
            <p class="m-0 rounded-lg border border-dashed border-slate-600/50 bg-slate-900/40 px-3 py-2 text-xs text-slate-400">
                {match filter_mode {
                    DiagnosticsFilterMode::CurrentFile => {
                        format!("当前文件 `{selected_target}` 无 compile diagnostics。")
                    }
                    DiagnosticsFilterMode::All => "本次编译无 compile diagnostics（不含 manage_page_pipeline）。".to_string(),
                }}
            </p>
        }
        .into_any()
    } else {
        view! { <></> }.into_any()
    };
    let other_hint = if filter_mode == DiagnosticsFilterMode::CurrentFile && other_count > 0 {
        let href = manage_tab_href(
            app_path,
            Some(selected_target),
            selected_target,
            true,
            ManageViewTab::Diagnostics,
            Some("all"),
        );
        view! {
            <p class="m-0 text-xs text-slate-400">
                {format!("另有 {other_count} 条诊断来自其它文件。")}
                <a class="ml-1 text-sky-300 hover:text-sky-200" href=href>"查看全部诊断"</a>
            </p>
        }
        .into_any()
    } else {
        view! { <></> }.into_any()
    };
    let href_current = manage_tab_href(
        app_path,
        Some(selected_target),
        selected_target,
        true,
        ManageViewTab::Diagnostics,
        None,
    );
    let href_all = manage_tab_href(
        app_path,
        Some(selected_target),
        selected_target,
        true,
        ManageViewTab::Diagnostics,
        Some("all"),
    );
    let filter_current_class = if filter_mode == DiagnosticsFilterMode::CurrentFile {
        "manage-diag-filter is-active"
    } else {
        "manage-diag-filter"
    };
    let filter_all_class = if filter_mode == DiagnosticsFilterMode::All {
        "manage-diag-filter is-active"
    } else {
        "manage-diag-filter"
    };
    view! {
        <section
            class="source-diagnostics mt-4 grid gap-2 border-t border-slate-600/40 pt-4"
            id="mei-manage-diagnostics-root"
            data-mei-diag-filter=filter_mode.slug()
            data-mei-selected-target=selected_target.to_string()
        >
            <div class="mb-0 grid gap-1">
                <h3 class="m-0 text-[15px] font-semibold text-slate-50">"错误与诊断"</h3>
                <p class="m-0 text-xs text-slate-400">
                    "编译期 diagnostics 默认按当前文件过滤；"
                    <code class="text-slate-200">"manage_page_pipeline"</code>
                    " 表示本页请求流水线。"
                    " 运行时见下方 "
                    <code class="text-slate-200">"layout_audit_runtime"</code>
                    " / "
                    <code class="text-slate-200">"runtime_query_errors"</code>
                    " / "
                    <code class="text-slate-200">"runtime_perf"</code>
                    " / "
                    <code class="text-slate-200">"agent_sse_delta"</code>
                    "。"
                </p>
                <div class="flex flex-wrap items-center gap-2 text-[11px]">
                    <span class="text-slate-500">"编译诊断范围："</span>
                    <a class=filter_current_class href=href_current data-mei-diag-filter-link="current">"当前文件"</a>
                    <a class=filter_all_class href=href_all data-mei-diag-filter-link="all">"全部诊断"</a>
                    <span class="text-slate-500">
                        {format!(
                            "（当前文件 {cur_e} 错 / {cur_w} 警 / {cur_i} 提 · 全部 {all_e} 错 / {all_w} 警 / {all_i} 提）"
                        )}
                    </span>
                </div>
                {other_hint}
            </div>
            {pipeline_views}
            <div class="grid gap-1" data-mei-compile-diagnostics="true">
                <span class="text-[10px] font-semibold uppercase tracking-wide text-slate-500">
                    "compile diagnostics"
                </span>
                {empty_compile_hint}
                {compile_cards}
            </div>
            <div class="diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-slate-900/35 border-cyan-500/25">
                <strong class="text-xs font-semibold text-slate-50">"layout_audit_runtime"</strong>
                <span class="text-xs leading-5 text-slate-300">
                    "预览几何审计：检测画布溢出、父容器裁切、零尺寸退化盒。"
                    " 内容由 "
                    <code class="text-slate-200">"frame-stage.js"</code>
                    " 上报并复用同一诊断面板。"
                </span>
                <div
                    id="mei-runtime-layout-audit"
                    data-empty-text="尚未发现布局几何问题。"
                    class="m-0 max-h-64 overflow-auto whitespace-pre-wrap break-words text-[11px] leading-5 text-slate-300"
                >
                    "尚未发现布局几何问题。"
                </div>
            </div>
            <div class="diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-slate-900/35 border-red-500/25">
                <strong class="text-xs font-semibold text-slate-50">"runtime_query_errors"</strong>
                <span class="text-xs leading-5 text-slate-300">
                    <code class="text-slate-200">"mei-dataset-table"</code>
                    " / "
                    <code class="text-slate-200">"dataset.summary-cards"</code>
                    " / "
                    <code class="text-slate-200">"chart.*"</code>
                    " 在请求失败时写入（最新 25 条）。"
                </span>
                <div
                    id="mei-runtime-query-errors"
                    class="m-0 max-h-64 overflow-auto whitespace-pre-wrap break-words text-[11px] leading-5 text-slate-300"
                >
                    "尚无 /api/datasets/query 或 metrics 运行时错误上报。"
                </div>
            </div>
            <div class="diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-slate-900/35 border-slate-500/35">
                <strong class="text-xs font-semibold text-slate-50">"runtime_perf"</strong>
                <span class="text-xs leading-5 text-slate-300">
                    "数据查询运行时耗时（最新 20 条；SPA 换文件会清空，避免其它页慢记录误导）。"
                    <code class="text-slate-200">"mei-dataset-table"</code>
                    " / "
                    <code class="text-slate-200">"dataset.summary-cards"</code>
                    " 对外部源会调 /api/datasets/query 或 metrics。"
                    <code class="text-slate-200">"client_ttfb_ms"</code>
                    " / "
                    <code class="text-slate-200">"client_json_ms"</code>
                    " 拆分浏览器侧；"
                    <code class="text-slate-200">"server_handler_total_ms"</code>
                    " 为接口 handler 墙钟；"
                    <code class="text-slate-200">"client_outside_server_ms"</code>
                    " 为二者差（排队、主线程、并发 manage 编译等）。"
                    " 行尾 "
                    <code class="text-slate-200">"scene/file"</code>
                    " 标明触发组件与当前预览目标。"
                </span>
                <div
                    id="runtime-perf-diagnostics"
                    class="m-0 max-h-56 overflow-auto whitespace-pre-wrap break-words text-[11px] leading-5 text-slate-300"
                >
                    "尚无懒加载查询记录。"
                </div>
            </div>
            <div class="diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-slate-900/35 border-emerald-500/25">
                <strong class="text-xs font-semibold text-slate-50">"agent_sse_delta"</strong>
                <span class="text-xs leading-5 text-slate-300">
                    "内置助手 EventSource "
                    <code class="text-slate-200">"message_part_delta"</code>
                    "："
                    <code class="text-slate-200">"srv"</code>
                    " 为载荷 "
                    <code class="text-slate-200">"server_ts_ms"</code>
                    "；"
                    <code class="text-slate-200">"cli_rx"</code>
                    " / "
                    <code class="text-slate-200">"gap_rx"</code>
                    " 为收到 SSE 并解析时的墙钟及对 srv 的差；"
                    <code class="text-slate-200">"cli_paint"</code>
                    " / "
                    <code class="text-slate-200">"gap_paint"</code>
                    " 为连续两次 requestAnimationFrame 之后（近似排帧后）及对 srv 的差。由作者面板写入；换文件 SPA 后请再点「调试」或收新 delta 以刷新。"
                </span>
                <div
                    id="mei-manage-debug-agent-sse-delta"
                    class="m-0 max-h-56 overflow-auto whitespace-pre-wrap break-words font-mono text-[11px] leading-5 text-slate-300"
                >
                    "尚无记录。连接作者会话后发消息；或从其它页签切回「调试」以刷新。"
                </div>
            </div>
        </section>
    }
    .into_any()
}
