use mei_lang_app::UiRouteMode;

use crate::http::pages::app::query::AppQuery;

pub(crate) const COMPILE_BOOTSTRAP_PROBE_DIAG_FILTER: &str = "__mei_compile_probe__";
pub(crate) const COMPILE_BOOTSTRAP_DISABLE_DIAG_FILTER: &str = "__mei_compile_no_bootstrap__";

pub(crate) fn compile_bootstrap_enabled() -> bool {
    if cfg!(test) {
        return false;
    }
    !crate::http::compile_cache::env_flag_enabled("MEI_DISABLE_COMPILE_BOOTSTRAP")
}

pub(crate) fn compile_bootstrap_probe_requested(query: &AppQuery) -> bool {
    query
        .diag_filter
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| value == COMPILE_BOOTSTRAP_PROBE_DIAG_FILTER)
}

pub(crate) fn compile_bootstrap_disabled_for_request(query: &AppQuery) -> bool {
    query
        .diag_filter
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| value == COMPILE_BOOTSTRAP_DISABLE_DIAG_FILTER)
}

/// Routes that may serve [`render_compiling_shell`] must also handle compile-bootstrap probes.
pub(crate) fn compile_bootstrap_route_supported(route_mode: UiRouteMode) -> bool {
    matches!(
        route_mode,
        UiRouteMode::Build | UiRouteMode::App | UiRouteMode::Presentation
    )
}

pub(crate) fn render_compiling_shell(
    _route_mode: UiRouteMode,
    app_id: &str,
    _scene_hint: Option<&str>,
) -> String {
    let app_esc = html_escape_min(app_id.trim_start_matches('/'));
    let probe_diag_filter = COMPILE_BOOTSTRAP_PROBE_DIAG_FILTER;
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <meta name="color-scheme" content="dark"/>
  <title>正在编译 · {app_esc}</title>
  <link rel="icon" href="/app-assets/favicon.svg"/>
  <link rel="stylesheet" href="/app-assets/dist/styles.bundle.css"/>
  <script src="/app-assets/page-load-progress-shell.js"></script>
</head>
<body data-mei-compile-shell="true">
  <script>
    (function () {{
      if (window.MeiPageLoadProgress) {{
        window.MeiPageLoadProgress.mountBootstrap();
      }}
      var baseHref = window.location.href;
      var nextDelayMs = 260;
      var maxDelayMs = 1200;
      var stopped = false;
      function computeNextDelay() {{
        var current = nextDelayMs;
        nextDelayMs = Math.min(maxDelayMs, Math.round(nextDelayMs * 1.35));
        return current;
      }}
      function buildProbeUrl() {{
        var probeUrl = new URL(baseHref, window.location.href);
        probeUrl.searchParams.set("diag_filter", "{probe_diag_filter}");
        probeUrl.searchParams.set("__mei_probe_ts", String(Date.now()));
        return probeUrl.toString();
      }}
      function schedule() {{
        if (stopped) return;
        window.setTimeout(tick, computeNextDelay());
      }}
      function doneReload() {{
        if (stopped) return;
        stopped = true;
        if (window.MeiPageLoadProgress) {{
          window.MeiPageLoadProgress.noteCompileReady();
        }}
        window.setTimeout(function () {{
          window.location.replace(baseHref);
        }}, 900);
      }}
      function tick() {{
        if (stopped) return;
        function probeReady(response) {{
          if (response.status === 204) return true;
          if (response.headers.get("x-mei-compile-bootstrap-ready") === "1") return true;
          return false;
        }}
        fetch(buildProbeUrl(), {{
          method: "GET",
          cache: "no-store",
          credentials: "same-origin",
          headers: {{
            "x-mei-compile-probe": "1"
          }}
        }})
          .then(function (response) {{
            var reason = response.headers.get("x-mei-compile-bootstrap-reason") || "";
            if (window.MeiPageLoadProgress) {{
              window.MeiPageLoadProgress.noteProbe(reason);
            }}
            if (probeReady(response)) {{
              doneReload();
              return;
            }}
            if (response.status >= 500) {{
              doneReload();
              return;
            }}
            if (response.status === 200) {{
              return response.text().then(function (html) {{
                if (!html.includes('data-mei-compile-shell="true"')) {{
                  doneReload();
                  return;
                }}
                schedule();
              }});
            }}
            schedule();
          }})
          .catch(function () {{
            if (window.MeiPageLoadProgress) {{
              window.MeiPageLoadProgress.noteProbe("network_retry");
            }}
            schedule();
          }});
      }}
      document.addEventListener("visibilitychange", function () {{
        if (document.visibilityState === "visible") {{
          nextDelayMs = 220;
        }}
      }});
      schedule();
    }})();
  </script>
</body>
</html>"#
    )
}

fn html_escape_min(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::pages::app::query::AppQuery;

    #[test]
    fn compiling_shell_escapes_app_id() {
        let html = render_compiling_shell(UiRouteMode::Build, "<bad>", None);
        assert!(html.contains("&lt;bad&gt;"));
        assert!(!html.contains("<bad>"));
    }

    #[test]
    fn compiling_shell_includes_progress_shell_assets() {
        let html = render_compiling_shell(UiRouteMode::Presentation, "zhifa", Some("home"));
        assert!(html.contains("page-load-progress-shell.js"));
        assert!(html.contains("MeiPageLoadProgress"));
        assert!(html.contains("data-mei-compile-shell=\"true\""));
        assert!(!html.contains("mei-compile-card"));
    }

    #[test]
    fn compile_bootstrap_route_support_matches_access_like_modes() {
        assert!(compile_bootstrap_route_supported(UiRouteMode::Build));
        assert!(compile_bootstrap_route_supported(UiRouteMode::App));
        assert!(compile_bootstrap_route_supported(UiRouteMode::Presentation));
        assert!(!compile_bootstrap_route_supported(UiRouteMode::Config));
        assert!(!compile_bootstrap_route_supported(UiRouteMode::Upload));
    }

    #[test]
    fn compile_bootstrap_probe_and_disable_flags_are_query_scoped() {
        let probe = AppQuery {
            file: None,
            scene: None,
            tab: None,
            diag_filter: Some(COMPILE_BOOTSTRAP_PROBE_DIAG_FILTER.to_string()),
            world_metric: None,
            world_dataset: None,
            explain: None,
            chrome: None,
        };
        assert!(compile_bootstrap_probe_requested(&probe));
        assert!(!compile_bootstrap_disabled_for_request(&probe));

        let disabled = AppQuery {
            file: None,
            scene: None,
            tab: None,
            diag_filter: Some(COMPILE_BOOTSTRAP_DISABLE_DIAG_FILTER.to_string()),
            world_metric: None,
            world_dataset: None,
            explain: None,
            chrome: None,
        };
        assert!(compile_bootstrap_disabled_for_request(&disabled));
        assert!(!compile_bootstrap_probe_requested(&disabled));
    }
}
