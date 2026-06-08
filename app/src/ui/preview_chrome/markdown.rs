use super::html_escape::{escape_html, escape_html_attr};

const MAX_LINES: usize = 800;

pub(crate) fn markdown_preview_html(source: &str) -> String {
    let lines: Vec<&str> = source.lines().take(MAX_LINES).collect();
    let mut html = String::new();
    let mut index = 0usize;
    let mut in_list = false;
    let mut in_code = false;

    while index < lines.len() {
        let line = lines[index].trim_end();
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if in_code {
                html.push_str("</code></pre>");
                in_code = false;
            } else {
                close_list(&mut html, &mut in_list);
                html.push_str("<pre><code>");
                in_code = true;
            }
            index += 1;
            continue;
        }
        if in_code {
            html.push_str(&escape_html(line));
            html.push('\n');
            index += 1;
            continue;
        }

        if let Some((table_html, consumed)) = try_parse_table_block(&lines, index) {
            close_list(&mut html, &mut in_list);
            html.push_str(&table_html);
            index += consumed;
            continue;
        }

        if trimmed.is_empty() {
            close_list(&mut html, &mut in_list);
            index += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            close_list(&mut html, &mut in_list);
            html.push_str("<h1>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h1>");
            index += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            close_list(&mut html, &mut in_list);
            html.push_str("<h2>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h2>");
            index += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            close_list(&mut html, &mut in_list);
            html.push_str("<h3>");
            html.push_str(&markdown_inline_html(rest));
            html.push_str("</h3>");
            index += 1;
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
            index += 1;
            continue;
        }

        close_list(&mut html, &mut in_list);
        html.push_str("<p>");
        html.push_str(&markdown_inline_html(trimmed));
        html.push_str("</p>");
        index += 1;
    }

    if in_code {
        html.push_str("</code></pre>");
    }
    close_list(&mut html, &mut in_list);
    if html.is_empty() {
        html.push_str("<p class=\"is-empty\">空文档</p>");
    }
    html
}

fn close_list(html: &mut String, in_list: &mut bool) {
    if *in_list {
        html.push_str("</ul>");
        *in_list = false;
    }
}

/// GFM 表格：表头行 + `| --- |` 分隔行 + 若干数据行。
fn try_parse_table_block(lines: &[&str], start: usize) -> Option<(String, usize)> {
    if start + 1 >= lines.len() {
        return None;
    }
    let header_line = lines[start].trim();
    let separator_line = lines[start + 1].trim();
    if !is_table_row(header_line) || !is_table_separator(separator_line) {
        return None;
    }
    let header = split_table_row(header_line);
    if header.is_empty() {
        return None;
    }
    let col_count = header.len();

    let mut body_rows: Vec<Vec<String>> = Vec::new();
    let mut index = start + 2;
    while index < lines.len() {
        let row_line = lines[index].trim();
        if row_line.is_empty() {
            break;
        }
        if !is_table_row(row_line) || is_table_separator(row_line) {
            break;
        }
        let mut cells = split_table_row(row_line);
        if cells.len() < col_count {
            cells.resize(col_count, String::new());
        } else if cells.len() > col_count {
            cells.truncate(col_count);
        }
        body_rows.push(cells);
        index += 1;
    }

    let consumed = index - start;
    if consumed < 2 {
        return None;
    }

    let mut html = String::from("<div class=\"md-table-wrap\"><table><thead><tr>");
    for cell in &header {
        html.push_str("<th>");
        html.push_str(&markdown_inline_html(cell));
        html.push_str("</th>");
    }
    html.push_str("</tr></thead>");
    if !body_rows.is_empty() {
        html.push_str("<tbody>");
        for row in &body_rows {
            html.push_str("<tr>");
            for cell in row {
                html.push_str("<td>");
                html.push_str(&markdown_inline_html(cell));
                html.push_str("</td>");
            }
            html.push_str("</tr>");
        }
        html.push_str("</tbody>");
    }
    html.push_str("</table></div>");
    Some((html, consumed))
}

fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty() && trimmed.contains('|')
}

fn is_table_separator(line: &str) -> bool {
    let cells = split_table_row(line);
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim();
            !cell.is_empty()
                && cell.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
                && cell.contains('-')
        })
}

fn split_table_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let mut inner = trimmed;
    if let Some(rest) = inner.strip_prefix('|') {
        inner = rest;
    }
    if let Some(rest) = inner.strip_suffix('|') {
        inner = rest;
    }
    inner
        .split('|')
        .map(str::trim)
        .map(ToString::to_string)
        .collect()
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

#[cfg(test)]
mod tests {
    use super::markdown_preview_html;

    #[test]
    fn renders_gfm_table() {
        let md = "| A | B |\n| --- | --- |\n| 1 | 2 |\n";
        let html = markdown_preview_html(md);
        assert!(html.contains("<table>"), "html={html}");
        assert!(html.contains("<th>"));
        assert!(html.contains("<td>"));
        assert!(html.contains("A"));
        assert!(html.contains("2"));
    }

    #[test]
    fn table_does_not_break_following_paragraph() {
        let md = "| H |\n| - |\n| x |\n\nAfter\n";
        let html = markdown_preview_html(md);
        assert!(html.contains("</table>"));
        assert!(html.contains("<p>After</p>"));
    }
}
