//! docker-compose-style log prefixes for host vs app runtimes.

use std::fmt;
use std::io::IsTerminal;

use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::{format::Writer, FmtContext, FormatEvent, FormatFields};
use tracing_subscriber::registry::LookupSpan;

const HOST_LABEL: &str = "host";
const HOST_ANSI: &str = "36"; // cyan

const APP_ANSI_PALETTE: &[&str] = &[
    "35", // magenta
    "33", // yellow
    "32", // green
    "34", // blue
    "31", // red
    "36", // cyan
    "95", // bright magenta
    "93", // bright yellow
];

pub(crate) fn supports_ansi_stderr() -> bool {
    std::io::stderr().is_terminal()
}

fn stable_app_ansi(app_id: &str) -> &'static str {
    let mut hash: u32 = 2166136261;
    for byte in app_id.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16777619);
    }
    APP_ANSI_PALETTE[(hash as usize) % APP_ANSI_PALETTE.len()]
}

pub(crate) fn format_service_prefix(label: &str, ansi: bool) -> String {
    let bracketed = format!("[{label}]");
    if !ansi {
        return format!("{bracketed:<16}");
    }
    let code = if label == HOST_LABEL {
        HOST_ANSI
    } else {
        stable_app_ansi(label)
    };
    // Pad plain width first so ANSI codes do not skew column alignment.
    let padded = format!("{bracketed:<16}");
    format!("\x1b[{code}m{padded}\x1b[0m")
}

pub(crate) fn emit_prefixed_line(label: &str, line: &str) {
    let prefix = format_service_prefix(label, supports_ansi_stderr());
    eprintln!("{prefix} {line}");
}

#[derive(Default)]
struct AppIdVisitor {
    app_id: Option<String>,
}

impl tracing::field::Visit for AppIdVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "app_id" {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.app_id = Some(trimmed.to_string());
            }
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        if field.name() == "app_id" && self.app_id.is_none() {
            let rendered = format!("{value:?}");
            let trimmed = rendered.trim().trim_matches('"');
            if !trimmed.is_empty() {
                self.app_id = Some(trimmed.to_string());
            }
        }
    }
}

pub(crate) struct ComposePrefixFormat {
    ansi: bool,
}

impl ComposePrefixFormat {
    pub(crate) fn new() -> Self {
        Self {
            ansi: supports_ansi_stderr(),
        }
    }
}

impl<S, N> FormatEvent<S, N> for ComposePrefixFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let mut visitor = AppIdVisitor::default();
        event.record(&mut visitor);
        let label = visitor
            .app_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(HOST_LABEL);
        write!(
            writer,
            "{} ",
            format_service_prefix(label, self.ansi)
        )?;

        let meta = event.metadata();
        if self.ansi {
            let level_color = match *meta.level() {
                tracing::Level::ERROR => "31",
                tracing::Level::WARN => "33",
                tracing::Level::INFO => "32",
                tracing::Level::DEBUG => "34",
                tracing::Level::TRACE => "90",
            };
            write!(
                writer,
                "\x1b[{level_color}m{:>5}\x1b[0m ",
                meta.level()
            )?;
        } else {
            write!(writer, "{:>5} ", meta.level())?;
        }

        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_prefix_is_fixed_width() {
        let plain = format_service_prefix("host", false);
        assert!(plain.starts_with("[host]"));
        assert_eq!(plain.len(), 16);
    }

    #[test]
    fn app_color_is_stable() {
        assert_eq!(stable_app_ansi("mei-tutorial"), stable_app_ansi("mei-tutorial"));
        assert_ne!(stable_app_ansi("a"), stable_app_ansi("zzzz"));
    }
}
