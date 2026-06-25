//! 访问态与演示态 preview 区共用布局 class，避免 shell 层 Tailwind 分叉导致视窗行为不一致。

pub(crate) fn access_shell_grid_class(chrome_hidden: bool, stage_enabled: bool) -> &'static str {
    if chrome_hidden {
        "shell shell-surface min-h-screen h-screen overflow-hidden max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "shell shell-surface grid min-h-screen h-screen overflow-hidden [grid-template-rows:auto_minmax(0,1fr)_auto] max-[1200px]:grid max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "shell shell-surface min-h-screen h-auto overflow-visible"
    }
}

pub(crate) fn access_main_preview_class(chrome_hidden: bool, stage_enabled: bool) -> &'static str {
    if chrome_hidden {
        "min-h-0 min-w-0 h-full overflow-hidden p-0 max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "min-h-0 min-w-0 h-full overflow-hidden p-4 self-stretch max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "min-w-0 h-auto overflow-visible p-4 self-start"
    }
}

pub(crate) fn access_preview_panel_class(chrome_hidden: bool, stage_enabled: bool) -> &'static str {
    if chrome_hidden {
        "min-h-0 min-w-0 h-full overflow-hidden [&_.preview-viewport]:h-full [&_.preview-viewport]:min-h-full [&_.preview-surface:not(.preview-stage)]:h-full [&_.preview-surface:not(.preview-stage)]:min-h-full max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else if stage_enabled {
        "min-h-0 min-w-0 h-full overflow-hidden [&_.preview-viewport-fluid-width]:max-h-full [&_.preview-viewport-fluid-width]:min-h-0 [&_.preview-viewport-fluid-width]:overflow-y-auto [&_.preview-surface]:min-h-auto max-[1200px]:h-auto max-[1200px]:overflow-visible"
    } else {
        "min-h-0 min-w-0 overflow-visible [&_.preview-surface]:min-h-auto"
    }
}

/// 演示态 preview 区与访问态 `?tab=preview` 对齐（无顶栏/底栏，但 preview 容器 class 相同）。
pub(crate) fn presentation_main_preview_class(stage_enabled: bool) -> String {
    concat_classes("relative", access_main_preview_class(false, stage_enabled))
}

pub(crate) fn presentation_preview_panel_class(stage_enabled: bool) -> &'static str {
    access_preview_panel_class(false, stage_enabled)
}

fn concat_classes(prefix: &str, base: &str) -> String {
    format!("{prefix} {base}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_preview_panel_matches_access_preview_tab() {
        assert_eq!(
            presentation_preview_panel_class(true),
            access_preview_panel_class(false, true)
        );
        assert_eq!(
            presentation_preview_panel_class(false),
            access_preview_panel_class(false, false)
        );
    }
}
