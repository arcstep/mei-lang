use super::prelude::*;

pub(crate) fn supports_ansi_stderr() -> bool {
    std::io::stderr().is_terminal()
}

pub(crate) fn ansi_wrap(text: &str, code: &str) -> String {
    if supports_ansi_stderr() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// Keep terminal/log lines readable: inline at most `max_lines`; longer payloads become a path hint.
pub(crate) fn format_log_blob(text: &str, max_lines: usize, path_hint: Option<&str>) -> String {
    let line_count = text.lines().count();
    if line_count <= max_lines {
        return text.to_string();
    }
    if let Some(path) = path_hint.filter(|value| !value.is_empty()) {
        return format!("<{line_count} lines omitted; see {path}>");
    }
    let preview: String = text.lines().take(max_lines).collect::<Vec<_>>().join("\n");
    format!(
        "{preview}\n... (+{} more lines)",
        line_count.saturating_sub(max_lines)
    )
}

pub(crate) fn emit_prebuild_status_line(status: &str, color_code: &str, detail: &str) {
    let rendered = format_log_blob(detail, 12, None);
    let prefix = ansi_wrap(status, color_code);
    eprintln!("{prefix} {rendered}");
    let _ = std::io::stderr().flush();
}
