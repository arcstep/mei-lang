pub(super) const STYLE: &str = r#"
* { box-sizing: border-box; }
body { margin: 0; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #0f172a; color: #e2e8f0; }
a { color: inherit; text-decoration: none; }
.shell { min-height: 100vh; display: grid; grid-template-rows: auto 1fr; }
.topbar { display: grid; grid-template-columns: 220px 1fr auto; gap: 16px; align-items: center; padding: 14px 20px; border-bottom: 1px solid rgba(148,163,184,.16); background: rgba(15,23,42,.92); position: sticky; top: 0; z-index: 10; }
.brand { display: grid; gap: 2px; }
.brand strong { font-size: 16px; }
.brand span { color: #94a3b8; font-size: 12px; }
.app-tabs, .mode-tabs { display: flex; flex-wrap: wrap; gap: 8px; }
.app-tab, .mode-tab { padding: 8px 12px; border: 1px solid rgba(96,165,250,.24); border-radius: 999px; background: rgba(30,41,59,.8); color: #cbd5e1; font-size: 13px; }
.app-tab.active, .mode-tab.active { background: rgba(37,99,235,.32); color: #eff6ff; border-color: rgba(96,165,250,.54); }
.workspace { min-height: 0; display: grid; grid-template-columns: 260px minmax(0, 1fr) 320px; gap: 16px; padding: 16px; }
.sidebar, .panel { border: 1px solid rgba(148,163,184,.14); border-radius: 16px; background: rgba(15,23,42,.78); }
.sidebar { padding: 14px; min-height: calc(100vh - 94px); overflow: auto; }
.main { min-width: 0; display: grid; gap: 16px; }
.panel { padding: 14px; }
.panel-heading { display: grid; gap: 4px; margin-bottom: 12px; }
.panel-heading h2, .panel-heading h3 { margin: 0; font-size: 15px; color: #f8fafc; }
.panel-heading p { margin: 0; color: #94a3b8; font-size: 12px; }
.preview-panel { min-height: 360px; }
.preview-surface { min-height: 100%; align-items: start; }
.preview-card { display: grid; gap: 10px; padding: 12px; border: 1px solid rgba(59,130,246,.18); border-radius: 14px; background: rgba(2,6,23,.32); }
.panel-body { display: grid; gap: 12px; min-width: 0; }
.component-card { min-width: 0; }
.component-host { min-height: 80px; }
.source-block { margin: 0; padding: 12px; border-radius: 12px; background: #020617; color: #cbd5e1; font-size: 12px; white-space: pre-wrap; overflow: auto; }
.tree { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
.tree-node details { padding-left: 4px; }
.tree-node summary { cursor: pointer; color: #cbd5e1; font-size: 13px; }
.tree-link { display: block; padding: 8px 10px; border-radius: 10px; color: #cbd5e1; font-size: 13px; background: rgba(30,41,59,.58); }
.tree-link.active { background: rgba(37,99,235,.28); color: #eff6ff; }
.opencode-placeholder { display: grid; gap: 10px; color: #cbd5e1; font-size: 13px; }
.opencode-placeholder ul { margin: 0; padding-left: 18px; color: #94a3b8; }
.diag { display: grid; gap: 4px; padding: 10px 12px; border-radius: 12px; margin-top: 8px; }
.diag-error { background: rgba(127,29,29,.25); border: 1px solid rgba(248,113,113,.28); }
.diag-warning { background: rgba(120,53,15,.22); border: 1px solid rgba(251,191,36,.28); }
.diag-info { background: rgba(30,64,175,.22); border: 1px solid rgba(96,165,250,.28); }
.scene-placeholder, .empty-preview { padding: 16px; border-radius: 14px; background: rgba(2,6,23,.36); border: 1px solid rgba(59,130,246,.18); }
.scene-placeholder h3 { margin: 0 0 8px; }
.scene-placeholder p, .empty-preview { color: #cbd5e1; }
.scene-placeholder ul { margin: 12px 0 0; padding-left: 18px; color: #94a3b8; }
@media (max-width: 1200px) {
  .workspace { grid-template-columns: 1fr; }
  .sidebar { min-height: 0; }
}
"#;
