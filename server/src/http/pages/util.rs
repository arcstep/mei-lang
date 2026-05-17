use std::time::Instant;

use mei_lang_kernel::{CompiledApp, Diagnostic, Severity};

pub(crate) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis() as u64
}

pub(crate) fn is_script_target(path: &str) -> bool {
    path.ends_with(".mei")
}

pub(crate) fn append_perf_diagnostic(
    compiled: &mut CompiledApp,
    target: &str,
    discover_ms: u64,
    compile_ms: u64,
    compile_cache_hit: bool,
    compile_cache_lookup_ms: u64,
    source_read_ms: u64,
) {
    let text = format!(
        "discover_apps_ms={discover_ms}ms | compile_ms={compile_ms}ms | compile_cache_hit={} | compile_cache_lookup_ms={compile_cache_lookup_ms}ms | source_read_ms={source_read_ms}ms | render_ms=__RENDER_MS__ | total_ms=__TOTAL_MS__",
        u8::from(compile_cache_hit)
    );
    compiled.diagnostics.push(Diagnostic {
        severity: Severity::Info,
        code: "perf_app_page".to_string(),
        message: format!("target={target} | {text}"),
        source_path: Some(target.to_string()),
    });
}

pub(crate) fn fill_perf_placeholders(mut html: String, render_ms: u64, total_ms: u64) -> String {
    html = html.replace("render_ms=__RENDER_MS__", format!("render_ms={render_ms}ms").as_str());
    html = html.replace("total_ms=__TOTAL_MS__", format!("total_ms={total_ms}ms").as_str());
    html
}
