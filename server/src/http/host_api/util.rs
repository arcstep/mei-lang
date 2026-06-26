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

pub(crate) fn emit_prebuild_status_line(status: &str, color_code: &str, detail: &str) {
    let prefix = ansi_wrap(status, color_code);
    eprintln!("{prefix} {detail}");
    let _ = std::io::stderr().flush();
}
