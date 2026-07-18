#!/usr/bin/env node
/** Apply shell/scene CSS separation to app-shell.css (idempotent). */
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const root = path.resolve(import.meta.dirname, "../..");
const appShellPath = path.join(root, "host-shell/app/assets/app-shell.css");
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

  /* Scene tiers T0/T1/T2 + P/C/Host planes (SSOT: docs/mei-lang-v2/03-ui/0301) */
  --mei-z-cockpit-map: 1;
  --mei-z-cockpit-panel: 1001;
  --mei-z-cockpit-header: 1110;
  --mei-z-cockpit-map-tools: 1210;
  --mei-z-cockpit-tooltip: 1300;
  --mei-z-drilldown: 2001;
  --mei-z-drilldown-board: 2010;
  --mei-z-layer2-workspace: 2001;
  --mei-z-drilldown-context: 2210;
  --mei-z-cockpit-tooltip-in-board: 2300;
  --mei-z-cockpit-text-popover: 2350;
  --mei-z-presentation-slide: 5000;
  --mei-z-presentation-caption: 5100;
  --mei-z-spa-loading: 5050;
  --mei-z-copilot-assistant: 5400;
  --mei-z-copilot-drawer: 5450;
  --mei-z-copilot-fab: 5500;
  --mei-z-copilot-fab-elevated: 5510;
  --mei-z-copilot-overlay: 5520;
  --mei-z-access-chat: 5410;
  --mei-z-access-chat-overlay: 5420;
  --mei-z-host-feedback: 5800;
  --mei-z-host-heartbeat: 5810;
  --mei-z-host-overlay: 5820;
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
