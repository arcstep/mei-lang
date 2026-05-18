use leptos::prelude::*;

use super::markdown::markdown_preview_html;

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
