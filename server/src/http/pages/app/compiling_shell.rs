use mei_lang_app::UiRouteMode;

pub(crate) fn compile_bootstrap_enabled() -> bool {
    if cfg!(test) {
        return false;
    }
    !crate::http::compile_cache::env_flag_enabled("MEI_DISABLE_COMPILE_BOOTSTRAP")
}

pub(crate) fn render_compiling_shell(
    route_mode: UiRouteMode,
    app_id: &str,
    scene_hint: Option<&str>,
) -> String {
    let app_esc = html_escape_min(app_id.trim_start_matches('/'));
    let mode_label = match route_mode {
        UiRouteMode::Build => "构建视图",
        UiRouteMode::App => "访问视图",
        UiRouteMode::Config => "配置视图",
        UiRouteMode::Upload => "上传视图",
    };
    let scene_line = scene_hint
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|scene| {
            let scene_esc = html_escape_min(scene);
            format!("<p class=\"mei-compile-scene\">场景 <code>{scene_esc}</code></p>")
        })
        .unwrap_or_default();
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
  <style>
    html, body {{
      margin: 0;
      min-height: 100%;
      background: #020617;
      color: #e2e8f0;
      font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
    }}
    .mei-compile-page {{
      min-height: 100vh;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 24px;
      box-sizing: border-box;
    }}
    .mei-compile-card {{
      max-width: 420px;
      width: 100%;
      padding: 20px 22px;
      border: 1px solid rgba(148, 163, 184, 0.28);
      border-radius: 14px;
      background: rgba(15, 23, 42, 0.96);
      box-shadow: 0 16px 40px rgba(2, 6, 23, 0.55);
    }}
    .mei-compile-head {{
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 12px;
    }}
    .mei-compile-head img {{
      width: 22px;
      height: 22px;
      animation: mei-spin 900ms linear infinite;
    }}
    .mei-compile-title {{
      margin: 0;
      font-size: 16px;
      font-weight: 600;
    }}
    .mei-compile-app {{
      margin: 0 0 8px;
      font-size: 13px;
      color: #94a3b8;
    }}
    .mei-compile-app code {{
      color: #cbd5e1;
    }}
    .mei-compile-scene {{
      margin: 0 0 8px;
      font-size: 13px;
      color: #94a3b8;
    }}
    .mei-compile-hint {{
      margin: 0;
      font-size: 13px;
      line-height: 1.55;
      color: #cbd5e1;
    }}
    .mei-compile-mode {{
      margin: 14px 0 0;
      font-size: 12px;
      color: #64748b;
    }}
  </style>
</head>
<body>
  <div class="mei-compile-page">
    <div class="mei-compile-card" role="status" aria-live="polite">
      <div class="mei-compile-head">
        <img src="/app-assets/favicon.svg" alt=""/>
        <h1 class="mei-compile-title">正在编译应用</h1>
      </div>
      <p class="mei-compile-app">应用 <code>{app_esc}</code> · {mode_label}</p>
      {scene_line}
      <p class="mei-compile-hint">首次打开或源码有更新时需要编译，请稍候。页面会在编译完成后自动刷新。</p>
      <p class="mei-compile-mode">MeiLang 编译引导页</p>
    </div>
  </div>
  <script>
    (function () {{
      var delayMs = 1500;
      window.setTimeout(function () {{
        window.location.reload();
      }}, delayMs);
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

    #[test]
    fn compiling_shell_escapes_app_id() {
        let html = render_compiling_shell(UiRouteMode::Build, "<bad>", None);
        assert!(html.contains("&lt;bad&gt;"));
        assert!(!html.contains("<bad>"));
    }
}
