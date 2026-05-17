use leptos::prelude::*;
use mei_lang_kernel::CompiledApp;

use super::route::UiRouteMode;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetPreviewKind {
    Markdown,
    Image,
    Pdf,
    Csv,
    Text,
    Unsupported,
}

pub(super) fn asset_preview_body(app_path: &str, target: &str, source: &str) -> AnyView {
    let kind = asset_preview_kind(target);
    asset_preview_inner(app_path, target, source, kind)
}

fn asset_preview_inner(
    app_path: &str,
    target: &str,
    source: &str,
    kind: AssetPreviewKind,
) -> AnyView {
    let asset_src = workspace_asset_href(app_path, target);
    let extension = target
        .rsplit('.')
        .next()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match kind {
        AssetPreviewKind::Markdown => {
            let html = markdown_preview_html(source);
            view! { <article class="asset-markdown-preview min-h-0 overflow-auto rounded-xl border border-slate-700/55 bg-slate-950/40 p-4" inner_html=html></article> }
                .into_any()
        }
        AssetPreviewKind::Image => {
            view! {
                <div class="asset-image-preview flex min-h-0 flex-1 items-center justify-center overflow-auto rounded-xl border border-slate-700/55 bg-slate-950/40 p-4">
                    <img class="max-h-full max-w-full rounded-lg object-contain" src=asset_src alt=target.to_string() loading="lazy"/>
                </div>
            }
            .into_any()
        }
        AssetPreviewKind::Pdf => {
            view! {
                <div class="asset-pdf-preview min-h-0 flex-1 overflow-hidden rounded-xl border border-slate-700/55 bg-slate-950/40">
                    <iframe class="h-full w-full border-0" src=asset_src title=target.to_string()></iframe>
                </div>
            }
            .into_any()
        }
        AssetPreviewKind::Csv => {
            let (headers, rows, truncated, shown_rows, shown_cols) = csv_preview_table(source, 120, 24);
            view! {
                <div class="asset-csv-preview grid min-h-0 flex-1 gap-2 overflow-hidden">
                    <div class="flex items-center justify-between gap-2 text-[11px] text-slate-400">
                        <span>{format!("CSV 预览：{} 行 · {} 列", shown_rows, shown_cols)}</span>
                        {if truncated {
                            view! { <span class="text-amber-300">"已截断显示"</span> }.into_any()
                        } else {
                            view! { <></> }.into_any()
                        }}
                    </div>
                    <div class="overflow-auto rounded-xl border border-slate-700/55 bg-slate-950/40">
                        <table class="min-w-full border-collapse text-left text-xs text-slate-200">
                            <thead class="sticky top-0 z-[1] bg-slate-900/95">
                                <tr>
                                    <th class="sticky left-0 z-[2] whitespace-nowrap border-b border-slate-700 bg-slate-900/95 px-3 py-2 font-semibold text-slate-400">"#"</th>
                                    {headers
                                        .iter()
                                        .enumerate()
                                        .map(|(idx, value)| {
                                            let title = if value.is_empty() {
                                                format!("列 {}", idx + 1)
                                            } else {
                                                value.clone()
                                            };
                                            view! {
                                                <th class="whitespace-nowrap border-b border-slate-700 px-3 py-2 font-semibold text-slate-100">{title}</th>
                                            }
                                        })
                                        .collect_view()}
                                </tr>
                            </thead>
                            <tbody>
                                {rows
                                    .iter()
                                    .enumerate()
                                    .map(|row| {
                                        let row_index = row.0 + 1;
                                        let row = row.1;
                                        view! {
                                            <tr>
                                                <td class="sticky left-0 z-[1] border-b border-slate-800/80 bg-slate-900/80 px-3 py-2 align-top text-slate-400">{row_index}</td>
                                                {row
                                                    .iter()
                                                    .map(|cell| {
                                                        view! { <td class="border-b border-slate-800/80 px-3 py-2 align-top leading-5 text-slate-300">{cell.clone()}</td> }
                                                    })
                                                    .collect_view()}
                                            </tr>
                                        }
                                    })
                                    .collect_view()}
                            </tbody>
                        </table>
                    </div>
                    {if truncated {
                        view! {
                            <div class="text-[11px] text-slate-400">
                                "CSV 预览已截断，仅展示前 120 行与前 24 列（含索引列）。"
                            </div>
                        }
                            .into_any()
                    } else {
                        view! { <></> }.into_any()
                    }}
                </div>
            }
            .into_any()
        }
        AssetPreviewKind::Text => {
            view! {
                <pre class="asset-text-preview min-h-0 flex-1 overflow-auto rounded-xl border border-slate-700/55 bg-slate-950/40 p-4 text-xs leading-6 text-slate-200">{source.to_string()}</pre>
            }
            .into_any()
        }
        AssetPreviewKind::Unsupported => {
            view! {
                <section class="grid min-h-0 flex-1 place-content-center gap-2 rounded-xl border border-dashed border-slate-600/55 bg-slate-950/35 p-6 text-center text-sm leading-6 text-slate-400">
                    <strong class="text-slate-100">"暂不支持该资源类型预览"</strong>
                    <span>{format!("目标：{}{}", target, if extension.is_empty() { "".to_string() } else { format!("（.{}）", extension) })}</span>
                </section>
            }
            .into_any()
        }
    }
}

fn asset_preview_kind(target: &str) -> AssetPreviewKind {
    let ext = target
        .rsplit('.')
        .next()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "md" | "markdown" => AssetPreviewKind::Markdown,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "avif" => AssetPreviewKind::Image,
        "pdf" => AssetPreviewKind::Pdf,
        "csv" => AssetPreviewKind::Csv,
        "txt" | "json" | "yaml" | "yml" | "toml" | "xml" | "log" | "rs" | "js" | "ts" | "tsx"
        | "jsx" | "css" | "html" | "htm" | "sh" | "zsh" | "bash" | "mei" | "star" => {
            AssetPreviewKind::Text
        }
        _ => {
            if ext.is_empty() {
                AssetPreviewKind::Text
            } else {
                AssetPreviewKind::Unsupported
            }
        }
    }
}

fn workspace_asset_href(app_path: &str, target: &str) -> String {
    format!(
        "/workspace-app-assets/{}/{}",
        percent_encode_path(app_path),
        percent_encode_path(target)
    )
}

fn percent_encode_path(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        let is_allowed =
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/');
        if is_allowed {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push_str(&format!("{:02X}", byte));
        }
    }
    output
}

fn csv_preview_table(
    source: &str,
    max_rows: usize,
    max_cols: usize,
) -> (Vec<String>, Vec<Vec<String>>, bool, usize, usize) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(source.as_bytes());
    let mut rows = Vec::new();
    let mut max_width = 0usize;
    let mut truncated = false;
    for record in reader.records() {
        match record {
            Ok(record) => {
                if rows.len() >= max_rows {
                    truncated = true;
                    break;
                }
                let mut row = record
                    .iter()
                    .take(max_cols)
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>();
                if record.len() > max_cols {
                    truncated = true;
                }
                max_width = max_width.max(row.len());
                if row.is_empty() {
                    row.push(String::new());
                }
                rows.push(row);
            }
            Err(_) => {
                return (
                    vec!["内容".to_string()],
                    source
                        .lines()
                        .take(max_rows)
                        .map(|line| vec![line.to_string()])
                        .collect::<Vec<_>>(),
                    source.lines().count() > max_rows,
                    source.lines().take(max_rows).count(),
                    1,
                );
            }
        }
    }
    if rows.is_empty() {
        return (
            vec!["内容".to_string()],
            vec![vec!["".to_string()]],
            false,
            1,
            1,
        );
    }
    let width = max_width.max(1);
    for row in &mut rows {
        while row.len() < width {
            row.push(String::new());
        }
    }
    let headers = (0..width)
        .map(|idx| rows[0].get(idx).cloned().unwrap_or_default())
        .collect::<Vec<_>>();
    let body = if rows.len() > 1 {
        rows[1..].to_vec()
    } else {
        Vec::new()
    };
    let shown_rows = body.len().max(1);
    (headers, body, truncated, shown_rows, width)
}

fn markdown_preview_html(source: &str) -> String {
    let mut html = String::new();
    let mut in_list = false;
    let mut in_code = false;
    for raw_line in source.lines().take(800) {
        let line = raw_line.trim_end();
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_code {
                html.push_str("</code></pre>");
                in_code = false;
            } else {
                if in_list {
                    html.push_str("</ul>");
                    in_list = false;
                }
                html.push_str("<pre><code>");
                in_code = true;
            }
            continue;
        }
        if in_code {
            html.push_str(&escape_html(line));
            html.push('\n');
            continue;
        }
        if trimmed.is_empty() {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str("<h1>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h1>");
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str("<h2>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h2>");
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            if in_list {
                html.push_str("</ul>");
                in_list = false;
            }
            html.push_str("<h3>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h3>");
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if !in_list {
                html.push_str("<ul>");
                in_list = true;
            }
            html.push_str("<li>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</li>");
            continue;
        }
        if in_list {
            html.push_str("</ul>");
            in_list = false;
        }
        html.push_str("<p>");
        html.push_str(&markdown_inline_html(trimmed));
        html.push_str("</p>");
    }
    if in_code {
        html.push_str("</code></pre>");
    }
    if in_list {
        html.push_str("</ul>");
    }
    if html.is_empty() {
        html.push_str("<p class=\"is-empty\">空文档</p>");
    }
    html
}

fn markdown_inline_html(value: &str) -> String {
    let mut output = String::new();
    let mut index = 0usize;
    while index < value.len() {
        let rest = &value[index..];
        let next_code = rest.find('`');
        let next_link = rest.find('[');
        let next_token = match (next_code, next_link) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let Some(next) = next_token else {
            output.push_str(&escape_html(rest));
            break;
        };
        if next > 0 {
            output.push_str(&escape_html(&rest[..next]));
            index += next;
            continue;
        }
        if rest.starts_with('`') {
            if let Some(end) = rest[1..].find('`') {
                let code = &rest[1..(1 + end)];
                output.push_str("<code>");
                output.push_str(&escape_html(code));
                output.push_str("</code>");
                index += end + 2;
            } else {
                output.push('`');
                index += 1;
            }
            continue;
        }
        if rest.starts_with('[') {
            if let Some(close) = rest.find(']') {
                let label = &rest[1..close];
                let remain = &rest[(close + 1)..];
                if let Some(link_body) = remain.strip_prefix('(') {
                    if let Some(end) = link_body.find(')') {
                        let raw_href = link_body[..end].trim();
                        if let Some(href) = sanitize_markdown_href(raw_href) {
                            output.push_str("<a href=\"");
                            output.push_str(&escape_html_attr(href));
                            output.push_str("\" target=\"_blank\" rel=\"noopener noreferrer\">");
                            output.push_str(&escape_html(label));
                            output.push_str("</a>");
                            index += close + end + 3;
                            continue;
                        }
                    }
                }
            }
            output.push('[');
            index += 1;
            continue;
        }
    }
    output
}

fn sanitize_markdown_href(raw: &str) -> Option<&str> {
    let href = raw.trim();
    if href.is_empty() {
        return None;
    }
    if href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
        || href.starts_with('/')
        || href.starts_with("./")
        || href.starts_with("../")
        || href.starts_with('#')
    {
        Some(href)
    } else {
        None
    }
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn manage_page_pipeline_diag_view(diag: &mei_lang_kernel::Diagnostic) -> AnyView {
    let base_class = "diag mt-2 grid gap-2 rounded-xl border px-3 py-2 bg-blue-900/25 border-blue-300/35";
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
                    "（由组件上报）。"
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
        </section>
    }
    .into_any()
}

pub(super) fn chrome_scripts_view(route_mode: UiRouteMode) -> AnyView {
    if route_mode == UiRouteMode::Manage {
        view! {
            <>
                <script src="/app-bundles/manage.js"></script>
            </>
        }
        .into_any()
    } else {
        view! {
            <>
                <script src="/app-bundles/access.js"></script>
            </>
        }
        .into_any()
    }
}

pub(super) fn component_scripts(compiled: &CompiledApp) -> impl IntoView {
    let scripts = compiled
        .component_assets
        .iter()
        .map(|asset| {
            let src = format!("/workspace-components/{}", asset.script);
            view! { <script type="module" src=src></script> }
        })
        .collect_view();
    view! { <>{scripts}</> }
}
