use std::io::{IsTerminal, Write};

fn supports_ansi_stdout() -> bool {
    std::io::stdout().is_terminal()
}

pub(crate) fn ansi_wrap(text: &str, code: &str) -> String {
    if supports_ansi_stdout() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn emit_banner(title: &str, detail_lines: &[&str], border_color: &str, title_color: &str) {
    const WIDTH: usize = 58;
    let border = "═".repeat(WIDTH);
    println!("{}", ansi_wrap(&border, border_color));
    println!("{}", ansi_wrap(&format!("  ✓ {title}"), title_color));
    for line in detail_lines {
        println!("  {line}");
    }
    println!("{}", ansi_wrap(&border, border_color));
    let _ = std::io::stdout().flush();
    tracing::info!(target: "mei.startup", title = %title, "host status banner");
    for line in detail_lines {
        tracing::info!(target: "mei.startup", "{line}");
    }
}

/// Port is open; HTTP handler accepting connections (early-bind or after blocking init).
pub(crate) fn emit_host_listening_banner(listen_url: &str, detail_lines: &[&str]) {
    let mut lines = vec![listen_url];
    lines.extend_from_slice(detail_lines);
    emit_banner(
        "服务已启动 · HOST LISTENING",
        lines.as_slice(),
        "1;36",
        "1;36;1",
    );
}

/// Import, plug-ds, bootstrap, and page-cache warmup finished — access pages may be served.
pub(crate) fn emit_access_warmup_ready_banner(detail_lines: &[&str]) {
    emit_banner(
        "访问态预热完成 · ACCESS READY",
        detail_lines,
        "1;32",
        "1;32;1",
    );
}

/// About to spawn `mei-app-runtime` for an app instance.
pub(crate) fn emit_app_start_banner(detail_lines: &[&str]) {
    emit_banner("应用启动 · APP START", detail_lines, "1;35", "1;35;1");
}

/// App runtime listen/health succeeded.
pub(crate) fn emit_app_ready_banner(detail_lines: &[&str]) {
    emit_banner("应用就绪 · APP READY", detail_lines, "1;32", "1;32;1");
}

/// App compile/import/warmup pipeline is starting.
pub(crate) fn emit_prebuild_start_banner(detail_lines: &[&str]) {
    emit_banner(
        "编译开始 · PREBUILD START",
        detail_lines,
        "1;33",
        "1;33;1",
    );
}

/// Workspace compile / import / plug-ds script finished (not the same as page ACCESS READY).
pub(crate) fn emit_prebuild_pipeline_complete_banner(detail_lines: &[&str]) {
    emit_banner(
        "编译流水线结束 · PREBUILD PIPELINE OK",
        detail_lines,
        "1;33",
        "1;33;1",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_wrap_without_tty_is_plain() {
        assert_eq!(ansi_wrap("ok", "1;32"), "ok");
    }
}
