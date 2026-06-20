#!/usr/bin/env node
/** Apply shell/scene CSS separation to app-shell.css (idempotent). */
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const root = path.resolve(import.meta.dirname, "..");
const appShellPath = path.join(root, "app/assets/app-shell.css");
const marker = "/* page-flow";

const ROOT_ALIAS = `:root {
  --mei-shell-stage-border: var(--mei-shell-color-border-default);
  --mei-surface-border: var(--mei-shell-color-border-default);
  --mei-surface-border-nav: var(--mei-shell-color-border-default);
  --mei-surface-border-main: var(--mei-shell-color-border-default);
  --mei-surface-border-tool: var(--mei-shell-color-border-default);
  --mei-divider-color: var(--mei-shell-color-border-default);
  --mei-surface-panel: var(--mei-shell-color-panel-bg);
  --mei-surface-panel-nav: var(--mei-shell-color-panel-bg);
  --mei-surface-panel-main: var(--mei-shell-color-panel-bg);
  --mei-surface-panel-tool: var(--mei-shell-color-panel-bg);
  --mei-surface-panel-header: var(--mei-shell-color-panel-bg);
  --mei-surface-panel-header-tool: var(--mei-shell-color-panel-bg);
  --mei-surface-panel-inset: none;
  --mei-status-chip-border: var(--mei-shell-color-border-default);
  --mei-status-chip-bg: var(--mei-shell-color-panel-bg);
  --mei-status-chip-text: var(--mei-shell-color-text-body);
  --mei-status-chip-good-border: var(--mei-shell-color-border-default);
  --mei-status-chip-good-bg: var(--mei-shell-color-panel-bg);
  --mei-status-chip-good-text: var(--mei-shell-color-text-body);
  --mei-status-chip-info-border: var(--mei-shell-color-border-default);
  --mei-status-chip-info-bg: var(--mei-shell-color-panel-bg);
  --mei-status-chip-info-text: var(--mei-shell-color-text-body);
  --mei-status-chip-warn-border: var(--mei-shell-color-border-default);
  --mei-status-chip-warn-bg: var(--mei-shell-color-panel-bg);
  --mei-status-chip-warn-text: var(--mei-shell-color-text-body);
  --mei-status-chip-danger-border: var(--mei-shell-color-border-default);
  --mei-status-chip-danger-bg: var(--mei-shell-color-panel-bg);
  --mei-status-chip-danger-text: var(--mei-shell-color-text-body);
  --mei-splitter-idle-line-left: var(--mei-shell-color-border-default);
  --mei-splitter-hover-line-left: var(--mei-shell-color-border-default);
  --mei-splitter-active-line-left: var(--mei-shell-color-border-default);
  --mei-splitter-active-track-left: var(--mei-shell-color-panel-bg);
  --mei-splitter-idle-line-right: var(--mei-shell-color-border-default);
  --mei-splitter-hover-line-right: var(--mei-shell-color-border-default);
  --mei-splitter-active-line-right: var(--mei-shell-color-border-default);
  --mei-splitter-active-track-right: var(--mei-shell-color-panel-bg);

  /* 首屏驾驶舱叠层（低→高）：底图 < 板块 < GIS工具 < 飘窗 < 二级看板 */
  --mei-z-cockpit-map: 1;
  --mei-z-cockpit-panel: 100;
  --mei-z-cockpit-header: 110;
  --mei-z-cockpit-map-tools: 1520;
  --mei-z-cockpit-tooltip: 1550;
  --mei-z-drilldown: 1600;
  --mei-z-drilldown-board: 1620;
  --mei-z-cockpit-tooltip-in-board: 1650;
  --mei-z-cockpit-text-popover: 1700;
}
`;

let raw = fs.readFileSync(appShellPath, "utf8");
const splitAt = raw.indexOf(marker);
if (splitAt === -1) throw new Error("page-flow marker not found");
let shell = raw.slice(0, splitAt);
const scene = raw.slice(splitAt);

// Replace :root block
shell = shell.replace(/^:root\s*\{[\s\S]*?\}\s*/m, ROOT_ALIAS);

// Shell token prefix migration
const shellColorMap = [
  ["--mei-color-text-primary", "--mei-shell-color-text-primary"],
  ["--mei-color-text-muted", "--mei-shell-color-text-muted"],
  ["--mei-color-text-body", "--mei-shell-color-text-body"],
  ["--mei-color-text-inverse", "--mei-shell-color-text-inverse"],
  ["--mei-color-text-accent", "--mei-shell-color-accent"],
  ["--mei-color-surface-bg", "--mei-shell-color-panel-bg"],
  ["--mei-color-border-default", "--mei-shell-color-border-default"],
];
for (const [from, to] of shellColorMap) shell = shell.replaceAll(from, to);
for (let n = 1; n <= 5; n++) {
  shell = shell.replaceAll(`--mei-font-${n}`, `--mei-shell-font-${n}`);
}
shell = shell.replace(
  /font-family:\s*Inter[^;]+;/,
  "font-family: var(--mei-shell-family-ui);",
);
shell = shell.replace(/var\((--mei-[^,)]+),\s*[^)]+\)/g, "var($1)");

shell = shell.replace(
  /\.mei-text-primary \{ color: var\(--mei-shell-color-text-primary\); \}/,
  ".mei-text-primary { color: var(--mei-shell-color-text-primary); }",
);
shell = shell.replace(
  /\.mei-text-muted \{ color: var\(--mei-shell-color-text-muted\); \}/,
  ".mei-text-muted { color: var(--mei-shell-color-text-muted); }",
);
shell = shell.replace(
  /\.mei-text-body \{ color: var\(--mei-shell-color-text-body\); \}/,
  ".mei-text-body { color: var(--mei-shell-color-text-body); }",
);
shell = shell.replace(
  /\.mei-text-inverse \{ color: var\(--mei-shell-color-text-inverse\); \}/,
  ".mei-text-inverse { color: var(--mei-shell-color-text-inverse); }",
);
shell = shell.replace(
  /\.mei-text-accent \{ color: var\(--mei-shell-color-accent\); \}/,
  ".mei-text-accent { color: var(--mei-shell-color-accent); }",
);

fs.writeFileSync(appShellPath, shell + scene);
console.log(`apply-shell-theme-css: shell=${shell.split("\n").length} scene=${scene.split("\n").length}`);

// materialize-shell-css-literals.mjs is deprecated (topic 33); use finalize-shell-theme.mjs instead.
const finalLines = fs.readFileSync(appShellPath, "utf8").split("\n").length;
if (finalLines < 5000) throw new Error(`app-shell.css too short (${finalLines} lines)`);
console.log(`apply-shell-theme-css: final ${finalLines} lines`);
