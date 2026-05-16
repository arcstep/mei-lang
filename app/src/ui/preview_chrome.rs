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

pub(super) fn asset_preview_view(app_path: &str, target: &str, source: &str) -> AnyView {
    let kind = asset_preview_kind(target);
    let asset_src = workspace_asset_href(app_path, target);
    let extension = target
        .rsplit('.')
        .next()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let content = match kind {
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
    };
    view! {
        <section class="asset-preview-pane h-full min-h-0" data-manage-tab-panel="preview">
            {content}
        </section>
    }
    .into_any()
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

pub(super) fn diagnostics_view(compiled: &CompiledApp) -> AnyView {
    if compiled.diagnostics.is_empty() {
        return view! { <></> }.into_any();
    }
    let diagnostics = compiled
        .diagnostics
        .iter()
        .map(|diag| {
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
        })
        .collect_view();
    view! {
        <section class="source-diagnostics mt-4 grid gap-2 border-t border-slate-600/40 pt-4">
            <div class="mb-0 grid gap-1">
                <h3 class="m-0 text-[15px] font-semibold text-slate-50">"编译提示"</h3>
                <p class="m-0 text-xs text-slate-400">"最小内核 diagnostics"</p>
            </div>
            {diagnostics}
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
