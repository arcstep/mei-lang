use leptos::prelude::*;
use mei_lang_kernel::{CompiledApp, CompiledSceneRoute, WorkspaceAppMeta};

use super::access_ai_entry::{external_access_ai_floating_entry, resolve_access_ai_external};
use super::manage_routing::WorldSemanticQuery;
use super::preview;
use super::scene_drilldown_context::host_ssr_bootstrap_scripts;
use super::shell_preview_layout::{
    access_shell_grid_class, presentation_main_preview_class, presentation_preview_panel_class,
};
use super::statusbar::statusbar_view;
use super::view_routing::app_scene_href;
use super::{HostAccountView, TopbarMenuContext};

const PRESENTATION_KEYBOARD_SCRIPT: &str = r#"
(function () {
  const shell = document.getElementById('presentation-shell');
  if (!shell || shell.dataset.bound === '1') return;
  shell.dataset.bound = '1';

  const go = (href) => {
    if (href) window.location.href = href;
  };

  const toggleFullscreen = async () => {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        await document.documentElement.requestFullscreen();
      }
    } catch (_) {}
  };

  document.addEventListener('keydown', (event) => {
    if (event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) return;
    const activeTag = document.activeElement && document.activeElement.tagName;
    if (activeTag === 'INPUT' || activeTag === 'TEXTAREA' || activeTag === 'SELECT') return;

    const prevHref = shell.dataset.prevHref || '';
    const nextHref = shell.dataset.nextHref || '';

    if ((event.key === 'ArrowLeft' || event.key === 'PageUp') && prevHref) {
      event.preventDefault();
      go(prevHref);
      return;
    }
    if ((event.key === 'ArrowRight' || event.key === 'PageDown' || event.key === ' ') && nextHref) {
      event.preventDefault();
      go(nextHref);
      return;
    }
    if ((event.key === 'f' || event.key === 'F') && !event.repeat) {
      event.preventDefault();
      toggleFullscreen();
      return;
    }
    if (event.key === 'Escape' && document.fullscreenElement) {
      event.preventDefault();
      document.exitFullscreen && document.exitFullscreen();
    }
  });

  const fullscreenButton = document.getElementById('presentation-fullscreen-btn');
  if (fullscreenButton) {
    fullscreenButton.addEventListener('click', (event) => {
      event.preventDefault();
      toggleFullscreen();
    });
  }
})();
"#;

fn exported_presentation_routes(compiled: &CompiledApp) -> Vec<&CompiledSceneRoute> {
    compiled
        .scene_routes
        .iter()
        .filter(|route| route.access_export)
        .collect()
}

fn current_presentation_scene<'a>(
    compiled: &'a CompiledApp,
    deck: &[&'a CompiledSceneRoute],
    selected_scene: Option<&str>,
) -> &'a str {
    let selected = selected_scene
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or(compiled.active_scene.as_deref())
        .unwrap_or(deck[0].scene_id.as_str());
    deck.iter()
        .find(|route| route.scene_id == selected)
        .map(|route| route.scene_id.as_str())
        .unwrap_or(deck[0].scene_id.as_str())
}

pub(crate) fn presentation_shell(
    _apps: &[WorkspaceAppMeta],
    compiled: &CompiledApp,
    app_path: &str,
    _topbar_menu: Option<&TopbarMenuContext>,
    selected_scene: Option<&str>,
    _file_target: Option<&str>,
    _source: Option<&str>,
    _active_tab: Option<&str>,
    _upload_enabled: bool,
    _auth_enabled: bool,
    _auth_account: Option<&HostAccountView>,
    data_mode: Option<&str>,
    review_projection: Option<&str>,
) -> AnyView {
    let deck = exported_presentation_routes(compiled);
    if deck.is_empty() {
        let statusbar = statusbar_view(app_path, "run", "", None);
        return view! {
            <div class="shell shell-surface presentation-shell min-h-screen mei-surface-shell mei-text-inverse">
                <main class="px-6 py-10">
                    <section class="mx-auto max-w-3xl rounded-3xl border mei-border-default mei-surface-panel-muted p-6 shadow-2xl">
                        <p class="text-sm uppercase tracking-[0.18em] mei-text-muted">"演说"</p>
                        <h1 class="mt-3 text-2xl font-semibold">{compiled.title.clone()}</h1>
                        <p class="mt-4 text-sm leading-7 mei-text-body">
                            "当前应用没有可用于演说的导出 scene。请先为至少一个 scene 保持默认 access export。"
                        </p>
                    </section>
                </main>
                {statusbar}
            </div>
        }
        .into_any();
    }

    let current_scene_id = current_presentation_scene(compiled, &deck, selected_scene);
    let current_index = deck
        .iter()
        .position(|route| route.scene_id == current_scene_id)
        .unwrap_or(0);
    let current_route = deck[current_index];
    let current_target = current_route.target_file.as_str();
    let external_ai = resolve_access_ai_external(compiled);
    let current_title = current_route
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(current_scene_id);
    let total = deck.len();
    let prev_href = current_index.checked_sub(1).map(|index| {
        app_scene_href(
            app_path,
            Some(deck[index].scene_id.as_str()),
            None,
            None,
            data_mode,
            review_projection,
        )
    });
    let next_href = deck.get(current_index + 1).map(|route| {
        app_scene_href(
            app_path,
            Some(route.scene_id.as_str()),
            None,
            None,
            data_mode,
            review_projection,
        )
    });
    let exit_href = app_scene_href(
        app_path,
        Some(current_scene_id),
        None,
        None,
        data_mode,
        review_projection,
    );
    let stage_enabled = preview::compiled_uses_frame_viewport(compiled);
    let shell_class = concat_presentation_shell_class(stage_enabled);
    let main_class = presentation_main_preview_class(stage_enabled);
    let preview_panel_class = presentation_preview_panel_class(stage_enabled);
    let preview = preview::preview_view(
        compiled,
        app_path,
        current_target,
        super::route::UiRouteMode::Run,
        WorldSemanticQuery::default(),
        None,
        None,
        data_mode,
        review_projection,
    );
    let statusbar = statusbar_view(app_path, "run", current_target, None);

    view! {
        <div
            id="presentation-shell"
            class=shell_class
            data-prev-href=prev_href.clone().unwrap_or_default()
            data-next-href=next_href.clone().unwrap_or_default()
        >
            {host_ssr_bootstrap_scripts(compiled, app_path, Some(current_scene_id), data_mode)}
            <main class=main_class>
                <section class=preview_panel_class>
                    {preview}
                </section>
                {external_ai
                    .as_ref()
                    .map(|entry| external_access_ai_floating_entry(app_path, entry))}

                <div class="pointer-events-none absolute inset-0 flex flex-col justify-between p-4 sm:p-6">
                    <header class="pointer-events-auto mx-auto flex w-full max-w-6xl items-center justify-between gap-3 rounded-2xl border border-white/10 mei-surface-panel-muted px-4 py-3 shadow-lg backdrop-blur-md">
                        <div class="min-w-0">
                            <div class="text-[11px] uppercase tracking-[0.18em] mei-text-muted">"演说"</div>
                            <div class="mt-1 truncate text-sm font-semibold mei-text-inverse">{compiled.title.clone()}</div>
                            <div class="truncate text-xs mei-text-muted">{current_title.to_string()}</div>
                        </div>
                        <div class="shrink-0 text-right">
                            <div class="text-xs mei-text-muted">"当前页"</div>
                            <div class="text-base font-semibold mei-text-inverse">{format!("{} / {}", current_index + 1, total)}</div>
                        </div>
                    </header>

                    <footer class="pointer-events-auto mx-auto flex w-full max-w-6xl items-center justify-between gap-3 rounded-2xl border border-white/10 mei-surface-panel-muted px-4 py-3 shadow-lg backdrop-blur-md">
                        <div class="flex items-center gap-2">
                            {prev_href
                                .as_ref()
                                .map(|href| {
                                    view! {
                                        <a class="inline-flex items-center rounded-xl border border-white/10 mei-surface-panel-muted px-3 py-2 text-sm mei-text-inverse transition hover:border-sky-300/60 hover:text-white" href=href.clone() rel="prev">
                                            "上一页"
                                        </a>
                                    }
                                    .into_any()
                                })
                                .unwrap_or_else(|| {
                                    view! {
                                        <span class="inline-flex cursor-not-allowed items-center rounded-xl border border-white/5 mei-surface-panel-muted px-3 py-2 text-sm mei-text-muted">
                                            "上一页"
                                        </span>
                                    }
                                    .into_any()
                                })}
                            {next_href
                                .as_ref()
                                .map(|href| {
                                    view! {
                                        <a class="inline-flex items-center rounded-xl border border-white/10 mei-surface-panel-muted px-3 py-2 text-sm mei-text-inverse transition hover:border-sky-300/60 hover:text-white" href=href.clone() rel="next">
                                            "下一页"
                                        </a>
                                    }
                                    .into_any()
                                })
                                .unwrap_or_else(|| {
                                    view! {
                                        <span class="inline-flex cursor-not-allowed items-center rounded-xl border border-white/5 mei-surface-panel-muted px-3 py-2 text-sm mei-text-muted">
                                            "下一页"
                                        </span>
                                    }
                                    .into_any()
                                })}
                        </div>

                        <div class="flex items-center gap-2">
                            <button
                                id="presentation-fullscreen-btn"
                                class="inline-flex items-center rounded-xl border border-white/10 mei-surface-panel-muted px-3 py-2 text-sm mei-text-inverse transition hover:border-sky-300/60 hover:text-white"
                                type="button"
                            >
                                "全屏"
                            </button>
                            <a class="inline-flex items-center rounded-xl border border-white/10 mei-surface-panel-muted px-3 py-2 text-sm mei-text-inverse transition hover:border-sky-300/60 hover:text-white" href=exit_href>
                                "返回访问页"
                            </a>
                        </div>
                    </footer>
                </div>
            </main>
            {statusbar}
            <script>{PRESENTATION_KEYBOARD_SCRIPT}</script>
        </div>
    }
    .into_any()
}

fn concat_presentation_shell_class(stage_enabled: bool) -> String {
    format!(
        "{} presentation-shell mei-surface-shell mei-text-inverse",
        access_shell_grid_class(true, stage_enabled)
    )
}
