use super::html_escape::{escape_html, escape_html_attr};

pub(super) fn markdown_preview_html(source: &str) -> String {
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
