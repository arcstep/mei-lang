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

fn emit_quiet_status(kind: &str, detail_lines: &[&str]) {
    let summary = if detail_lines.is_empty() {
        kind.to_string()
    } else {
        format!("{kind} · {}", detail_lines.join(" · "))
    };
    println!("{summary}");
    let _ = std::io::stdout().flush();
    tracing::info!(target: "mei.startup", kind = %kind, "{summary}");
}

fn emit_ready_banner(title: &str, detail_lines: &[&str]) {
    const WIDTH: usize = 58;
    let border = "═".repeat(WIDTH);
    println!("{}", ansi_wrap(&border, "1;32"));
    println!("{}", ansi_wrap(&format!("  ✓ {title}"), "1;32;1"));
    for line in detail_lines {
        println!("  {line}");
    }
    println!("{}", ansi_wrap(&border, "1;32"));
    let _ = std::io::stdout().flush();
    // Single structured log — avoid duplicating each detail line as [host] INFO.
    tracing::info!(
        target: "mei.startup",
        title = %title,
        details = %detail_lines.join(" | "),
        "APP READY"
    );
}

/// Port is open; HTTP handler accepting connections (early-bind or after blocking init).
pub(crate) fn emit_host_listening_banner(listen_url: &str, detail_lines: &[&str]) {
    let mut lines = vec![listen_url];
    lines.extend_from_slice(detail_lines);
    emit_quiet_status("HOST LISTENING", lines.as_slice());
}

/// Import, plug-ds, bootstrap, and page-cache warmup finished — access pages may be served.
pub(crate) fn emit_access_warmup_ready_banner(detail_lines: &[&str]) {
    emit_quiet_status("ACCESS READY", detail_lines);
}

/// About to spawn `mei-app-runtime` for an app instance.
pub(crate) fn emit_app_start_banner(detail_lines: &[&str]) {
    emit_quiet_status("APP START", detail_lines);
}

/// App runtime listen/health succeeded — the only decorated startup banner.
pub(crate) fn emit_app_ready_banner(detail_lines: &[&str]) {
    emit_ready_banner("应用就绪 · APP READY", detail_lines);
}

/// App compile/import/warmup pipeline is starting.
pub(crate) fn emit_prebuild_start_banner(detail_lines: &[&str]) {
    emit_quiet_status("PREBUILD START", detail_lines);
}

/// Workspace compile / import / plug-ds script finished (not the same as page ACCESS READY).
pub(crate) fn emit_prebuild_pipeline_complete_banner(detail_lines: &[&str]) {
    emit_quiet_status("PREBUILD PIPELINE OK", detail_lines);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_wrap_without_tty_is_plain() {
        assert_eq!(ansi_wrap("ok", "1;32"), "ok");
    }
}
