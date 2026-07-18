#!/usr/bin/env node
/**
 * Shell theme semantic cleanup: rename literal_* → semantic keys, prune dead keys,
 * rewrite shell CSS, sync workspace fragment + workspaces.
 */
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const root = path.resolve(import.meta.dirname, "../..");
const mapPath = path.join(root, "scripts/theme/shell-theme-semantic-map.json");
const fragmentPath = path.join(root, "scripts/theme/workspace-host-theme.fragment.json");
const appShellPath = path.join(root, "host-shell/app/assets/app-shell.css");
const hostShellPath = path.join(root, "host-shell/app/assets/host-shell.css");
const workspacePaths = [
  path.join(root, "../workspaces/ws-spbjw/.mei-workspace.json"),
  path.join(root, "../workspaces/ws-dev/.mei-workspace.json"),
  path.join(root, "../workspaces/ws-hello/.mei-workspace.json"),
];

const { literal_to_semantic, drop_literals, css_special } = JSON.parse(
  fs.readFileSync(mapPath, "utf8"),
);

function literalVar(literalKey) {
  return `--mei-shell-color-${literalKey}`;
}

function semanticVar(semanticKey) {
  return `--mei-shell-color-${semanticKey.replace(/_/g, "-")}`;
}

function readBaselineFragment() {
  try {
    const raw = execSync("git show HEAD:scripts/theme/workspace-host-theme.fragment.json", {
      cwd: root,
      encoding: "utf8",
    });
    return JSON.parse(raw);
  } catch {
    return JSON.parse(fs.readFileSync(fragmentPath, "utf8"));
  }
}

function normalizeShellColorVars(text) {
  return text.replace(/--mei-shell-color-([a-z0-9_-]+)/g, (full, key) => {
    if (!key.includes("_")) return full;
    return `--mei-shell-color-${key.replace(/_/g, "-")}`;
  });
}

function rewriteShellCssText(text) {
  let out = text;
  out = out.replace(
    /background:\s*linear-gradient\(180deg,\s*var\(--mei-shell-color-literal_61084d59\),\s*var\(--mei-shell-color-literal_b8f07ab1\)\);/g,
    `background: var(${semanticVar("manage_panel_bg")});`,
  );
  out = out.replace(
    /background:\s*linear-gradient\(180deg,\s*var\(--mei-shell-color-literal_5b742ad8\),\s*var\(--mei-shell-color-splitter[-_]grad[-_]b\)\);/g,
    `background: var(${semanticVar("chip_file_icon_bg")});`,
  );
  out = out.replace(
    /background:\s*\n\s*var\(--mei-shell-color-literal_e90d16ca\)[^;]+;/g,
    `background: var(${semanticVar("splitter_rail_grad")});`,
  );
  for (const [literal, semantic] of Object.entries(literal_to_semantic)) {
    out = out.split(literalVar(literal)).join(semanticVar(semantic));
  }
  const fixes = [
    ["border_default", "border-default"],
    ["panel_bg", "panel-bg"],
    ["text_primary", "text-primary"],
    ["text_muted", "text-muted"],
    ["text_body", "text-body"],
    ["text_inverse", "text-inverse"],
    ["splitter_grad_b", "splitter-grad-b"],
    ["splitter_line", "splitter-line"],
    ["border_accent_soft", "border-accent-soft"],
    ["border_muted", "border-muted"],
    ["border_strong", "border-strong"],
    ["border_faint", "border-faint"],
    ["border_slate", "border-slate"],
    ["border_slate_soft", "border-slate-soft"],
    ["shadow_deep", "shadow-deep"],
    ["progress_bar", "progress-bar"],
    ["glow_indigo", "glow-indigo"],
    ["glow_teal", "glow-teal"],
    ["glow_warn", "glow-warn"],
    ["glow_danger", "glow-danger"],
    ["inset_highlight", "inset-highlight"],
  ];
  for (const [wrong, right] of fixes) {
    out = out.split(`--mei-shell-color-${wrong}`).join(`--mei-shell-color-${right}`);
  }
  return normalizeShellColorVars(out);
}

function collectUsedKeys(...texts) {
  const used = new Set();
  for (const text of texts) {
    for (const m of text.matchAll(/--mei-shell-color-([\w-]+)/g)) {
      used.add(m[1].replace(/-/g, "_"));
    }
  }
  return used;
}

function buildFinalColor(oldColor) {
  const next = {};
  for (const [key, value] of Object.entries(oldColor)) {
    if (key.includes("-")) continue;
    if (key.startsWith("literal_")) {
      if (drop_literals.includes(key)) continue;
      const semantic = literal_to_semantic[key];
      if (semantic) next[semantic] = css_special[semantic] ?? value;
      continue;
    }
    next[key] = value;
  }
  for (const [key, value] of Object.entries(css_special)) {
    next[key] = value;
  }
  return next;
}

const appShell = rewriteShellCssText(fs.readFileSync(appShellPath, "utf8"));
const hostShell = rewriteShellCssText(fs.readFileSync(hostShellPath, "utf8"));
fs.writeFileSync(appShellPath, appShell);
fs.writeFileSync(hostShellPath, hostShell);

const used = collectUsedKeys(appShell, hostShell);
const fragment = readBaselineFragment();
let finalColor = buildFinalColor(fragment.themes.host.tokens.color);
finalColor = Object.fromEntries(Object.entries(finalColor).filter(([key]) => used.has(key)));
if (fragment.themes.host.tokens.color.watermark) {
  finalColor.watermark = fragment.themes.host.tokens.color.watermark;
}
finalColor = Object.fromEntries(
  Object.keys(finalColor)
    .sort()
    .map((k) => [k, finalColor[k]]),
);

fragment.themes.host.tokens.color = finalColor;
fs.writeFileSync(fragmentPath, JSON.stringify(fragment, null, 2) + "\n");

for (const wsPath of workspacePaths) {
  if (!fs.existsSync(wsPath)) continue;
  const ws = JSON.parse(fs.readFileSync(wsPath, "utf8"));
  ws.ops = ws.ops || {};
  ws.ops.shellTheme = fragment.shellTheme;
  ws.ops.themes = { ...(ws.ops.themes || {}), host: fragment.themes.host };
  fs.writeFileSync(wsPath, JSON.stringify(ws, null, 2) + "\n");
}

const shellLiteralLeft = (appShell + hostShell).match(/--mei-shell-color-literal_/g)?.length ?? 0;
const jsonLiteralLeft = Object.keys(finalColor).filter((k) => k.startsWith("literal_"));
console.log(
  `finalize-shell-theme: ${Object.keys(finalColor).length} color keys, css literal refs=${shellLiteralLeft}`,
);
if (shellLiteralLeft || jsonLiteralLeft.length) {
  console.error("remaining literals in json:", jsonLiteralLeft);
  process.exit(1);
}
