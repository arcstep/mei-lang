#!/usr/bin/env node
/**
 * One-shot: scan shell chrome CSS (before /* page-flow), map literals to
 * tokens.color keys, update workspace fragment + replace in app-shell.css / host-shell.css.
 */
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";

const root = path.resolve(import.meta.dirname, "..");
const appShellPath = path.join(root, "app/assets/app-shell.css");
const hostShellPath = path.join(root, "app/assets/host-shell.css");
const fragmentPath = path.join(root, "scripts/workspace-host-theme.fragment.json");
const workspacePaths = [
  path.join(root, "../workspaces/ws-spbjw/.mei-workspace.json"),
  path.join(root, "../workspaces/ws-dev/.mei-workspace.json"),
  path.join(root, "../workspaces/ws-hello/.mei-workspace.json"),
];

const SEMANTIC = new Map([
  ["rgba(100, 116, 139, 0.2)", "border_muted"],
  ["rgba(100, 116, 139, 0.28)", "border_strong"],
  ["rgba(100, 116, 139, 0.18)", "border_faint"],
  ["rgba(148, 163, 184, 0.28)", "border_slate_soft"],
  ["rgba(148, 163, 184, 0.22)", "border_slate"],
  ["rgba(148, 163, 184, 0.42)", "splitter_line"],
  ["rgba(96, 165, 250, 0.18)", "border_accent_soft"],
  ["rgba(96, 165, 250, 0.16)", "border_default"],
  ["rgba(96, 165, 250, 0.24)", "chrome_border_top"],
  ["rgba(2, 6, 23, 0.45)", "shadow_deep"],
  ["rgba(2, 8, 23, 0.32)", "overlay_dark"],
  ["rgba(2, 8, 23, 0.35)", "shadow_card"],
  ["rgba(2, 8, 23, 0.45)", "shadow_banner"],
  ["rgba(129, 140, 248, 0.18)", "glow_indigo"],
  ["rgba(226, 232, 240, 0.06)", "inset_highlight"],
  ["rgba(226, 232, 240, 0.42)", "splitter_grad_a"],
  ["rgba(148, 163, 184, 0.3)", "splitter_grad_b"],
  ["rgba(45, 212, 191, 0.16)", "glow_teal"],
  ["rgba(251, 191, 36, 0.2)", "glow_warn"],
  ["rgba(248, 113, 113, 0.22)", "glow_danger"],
  ["rgba(248, 113, 113, 0.35)", "banner_danger_border"],
  ["rgba(251, 191, 36, 0.35)", "banner_warn_border"],
  ["rgba(251, 113, 133, 0.11)", "watermark"],
  ["#bfdbfe", "status_info"],
  ["#fbbf24", "status_warn"],
  ["#f87171", "status_danger"],
  ["#38bdf8", "accent_sky"],
  ["#60a5fa", "accent_blue"],
  ["#34d399", "accent_emerald"],
  ["#94a3b8", "text_muted"],
  ["#cbd5e1", "text_body"],
  ["#f8fafc", "text_inverse"],
  ["#fde68a", "code"],
  ["#7dd3fc", "link"],
  ["#86efac", "feedback_ok"],
  ["#041320", "btn_primary_text"],
  ["#0ea5e9", "btn_primary_solid"],
  ["rgba(17, 26, 44, 0.6)", "card_bg"],
  ["rgba(15, 23, 42, 0.35)", "hint_bg"],
  ["rgba(15, 23, 42, 0.45)", "btn_secondary_bg"],
  ["rgba(15, 23, 42, 0.55)", "input_bg"],
  ["rgba(15, 23, 42, 0.96)", "banner_bg"],
  ["rgba(51, 65, 85, 0.45)", "hint_border"],
  ["rgba(51, 65, 85, 0.55)", "btn_border"],
  ["rgba(51, 65, 85, 0.65)", "input_border"],
  ["rgba(14, 165, 233, 0.55)", "focus_border"],
  ["rgba(14, 165, 233, 0.12)", "focus_ring"],
  ["rgba(14, 165, 233, 0.45)", "focus_ring_strong"],
  [
    "linear-gradient(90deg, #38bdf8, #60a5fa, #34d399)",
    "progress_bar",
  ],
  [
    "linear-gradient(180deg, rgba(12, 18, 31, 0.92), rgba(2, 6, 23, 0.76))",
    "panel_elevated_bg",
  ],
  [
    "linear-gradient(180deg, #38bdf8 0%, #0ea5e9 100%)",
    "btn_primary_bg",
  ],
  [
    "linear-gradient(180deg, rgba(6, 12, 24, 0.99), rgba(11, 18, 29, 0.97))",
    "manage_header_bg",
  ],
  [
    "linear-gradient(180deg, rgba(226, 232, 240, 0.42), rgba(148, 163, 184, 0.3))",
    "splitter_idle_grad",
  ],
]);

const literalRe =
  /#[0-9a-fA-F]{3,8}\b|rgba?\([^)]+\)|hsla?\([^)]+\)|linear-gradient\([^;)"']+(?:\([^)]*\)[^;)"']*)*\)/g;

function tokenName(literal, existing) {
  if (SEMANTIC.has(literal)) return SEMANTIC.get(literal);
  const hash = crypto.createHash("md5").update(literal).digest("hex").slice(0, 8);
  let name = `literal_${hash}`;
  let i = 0;
  while ([...existing.values()].includes(name)) {
    i += 1;
    name = `literal_${hash}_${i}`;
  }
  return name;
}

function stripVarFallbacks(css) {
  return css.replace(/var\((--mei-[^,)]+),\s*[^)]+\)/g, "var($1)");
}

function processFile(css, colorMap, { inRootSkip = true } = {}) {
  const lines = css.split("\n");
  let inRoot = false;
  let rootDepth = 0;
  const out = [];
  for (const line of lines) {
    let next = line;
    if (inRootSkip && /^:root\s*\{/.test(line.trim())) {
      inRoot = true;
      rootDepth = 1;
      out.push(next);
      continue;
    }
    if (inRoot) {
      rootDepth += (line.match(/\{/g) || []).length;
      rootDepth -= (line.match(/\}/g) || []).length;
      if (rootDepth <= 0) inRoot = false;
      out.push(next);
      continue;
    }
    next = stripVarFallbacks(next);
    literalRe.lastIndex = 0;
    next = next.replace(literalRe, (lit) => {
      if (!colorMap.has(lit)) {
        const name = tokenName(lit, colorMap);
        colorMap.set(lit, name);
      }
      return `var(--mei-shell-color-${colorMap.get(lit)})`;
    });
    out.push(next);
  }
  return out.join("\n");
}

function loadFragmentColors() {
  const fragment = JSON.parse(fs.readFileSync(fragmentPath, "utf8"));
  const color = fragment.themes.host.tokens.color;
  const map = new Map();
  for (const [k, v] of Object.entries(color)) {
    map.set(v, k);
  }
  return { fragment, color, map };
}

const { fragment, color, map: colorMap } = loadFragmentColors();

let appRaw = fs.readFileSync(appShellPath, "utf8");
const marker = "/* page-flow";
const splitAt = appRaw.indexOf(marker);
const shellPart = appRaw.slice(0, splitAt);
const scenePart = appRaw.slice(splitAt);

let shellFixed = shellPart;
shellFixed = processFile(shellFixed, colorMap);
shellFixed = shellFixed.replace(
  /\.mei-surface-panel \{ background: var\(--mei-surface-panel-main, var\(--mei-shell-color-panel-bg\)\); \}/,
  ".mei-surface-panel { background: var(--mei-surface-panel-main); }",
);
shellFixed = shellFixed.replace(
  /\.mei-border-muted \{ border-color: var\(--mei-surface-border-nav, var\(--mei-surface-border\)\); \}/,
  ".mei-border-muted { border-color: var(--mei-surface-border-nav); }",
);

let hostCss = fs.readFileSync(hostShellPath, "utf8");
hostCss = processFile(hostCss, colorMap, { inRootSkip: false });
hostCss = hostCss
  .replace(/font-size: 11px/g, "font-size: var(--mei-shell-font-1)")
  .replace(/font-size: 12px/g, "font-size: var(--mei-shell-font-2)")
  .replace(/font-size: 13px/g, "font-size: var(--mei-shell-font-2)")
  .replace(/font-size: 14px/g, "font-size: var(--mei-shell-font-3)")
  .replace(/font-size: 22px/g, "font-size: var(--mei-shell-font-4)")
  .replace(/font-size: 44px/g, "font-size: var(--mei-shell-font-4)")
  .replace(/font-size: 18px/g, "font-size: var(--mei-shell-font-3)")
  .replace(/font-size: 16px/g, "font-size: var(--mei-shell-font-3)");

for (const [literal, name] of colorMap) {
  if (!(name in color)) color[name] = literal;
}

fs.writeFileSync(appShellPath, shellFixed + scenePart);
fs.writeFileSync(hostShellPath, hostCss);
fs.writeFileSync(fragmentPath, JSON.stringify(fragment, null, 2) + "\n");

for (const wsPath of workspacePaths) {
  if (!fs.existsSync(wsPath)) continue;
  const ws = JSON.parse(fs.readFileSync(wsPath, "utf8"));
  ws.ops = ws.ops || {};
  ws.ops.shellTheme = fragment.shellTheme;
  ws.ops.themes = { ...(ws.ops.themes || {}), ...fragment.themes };
  fs.writeFileSync(wsPath, JSON.stringify(ws, null, 2) + "\n");
}

console.log(`materialized ${colorMap.size} shell color tokens`);
