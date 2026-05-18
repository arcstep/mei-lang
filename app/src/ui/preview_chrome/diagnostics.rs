use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::html_escape::escape_html_attr;

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

pub(super) fn diagnostics_view(compiled: &CompiledApp) -> AnyView {
    if compiled.diagnostics.is_empty() {
        return view! { <></> }.into_any();
    }
    let diagnostics = compiled
        .diagnostics
        .iter()
        .map(|diag| {
            if diag.code == "manage_page_pipeline" {
                manage_page_pipeline_diag_view(diag)
            } else {
                let class = match diag.severity {
                    mei_lang_kernel::Severity::Error => {
                        "diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-red-900/25 border-red-400/30"
                    }
                    mei_lang_kernel::Severity::Warning => {
                        "diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-amber-900/25 border-amber-300/35"
                    }
                    mei_lang_kernel::Severity::Info => {
                        "diag mt-2 grid gap-1 rounded-xl border px-3 py-2 bg-blue-900/25 border-blue-300/35"
                    }
                };
                view! {
                    <div class=class>
                        <strong class="text-xs font-semibold text-slate-50">{diag.code.clone()}</strong>
                        <span class="text-xs leading-5 text-slate-200">{diag.message.clone()}</span>
                    </div>
                }
                .into_any()
            }
        })
        .collect_view();
    view! {
        <section class="source-diagnostics mt-4 grid gap-2 border-t border-slate-600/40 pt-4">
            <div class="mb-0 grid gap-1">
                <h3 class="m-0 text-[15px] font-semibold text-slate-50">"错误与诊断"</h3>
                <p class="m-0 text-xs text-slate-400">
                    "编译期 diagnostics；管理页加载流水线见 "
                    <code class="text-slate-200">"manage_page_pipeline"</code>
                    "（JSON）。运行时 "
                    <code class="text-slate-200">"/api/datasets/query"</code>
                    " / metrics 失败见下方 "
                    <code class="text-slate-200">"runtime_query_errors"</code>
                    "（由组件上报）。内置助手 SSE 流式 delta 见 "
                    <code class="text-slate-200">"agent_sse_delta"</code>
                    "（由作者面板写入，与事件 JSON 字段一致）。"
                </p>
            </div>
            {diagnostics}
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
                    "数据查询运行时耗时（最新 20 条）。"
                    <code class="text-slate-200">"mei-dataset-table"</code>
                    " 对外部 csv/json/xlsx/db 源会调 /api/datasets/query；派生 "
                    <code class="text-slate-200">"dataset_view"</code>
                    " 无独立文件，只用编译期物化的 rows；编译阶段耗时见 "
                    <code class="text-slate-200">"manage_page_pipeline"</code>
                    " JSON 中 "
                    <code class="text-slate-200">"compile_app"</code>
                    " 阶段。"
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
