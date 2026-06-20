#!/usr/bin/env node
/**
 * Migrate shell chrome CSS to --mei-shell-* / --mei-shell-color-* / --mei-shell-font-*.
 * Scene tokens inside .preview-viewport are unchanged.
 */
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");

function migrateShellSection(css, endMarker) {
  const end = css.indexOf(endMarker);
  const shellPart = end === -1 ? css : css.slice(0, end);
  const rest = end === -1 ? "" : css.slice(end);
  let out = shellPart;
  const shellColorMap = [
    ["--mei-color-text-primary", "--mei-shell-color-text-primary"],
    ["--mei-color-text-muted", "--mei-shell-color-text-muted"],
    ["--mei-color-text-body", "--mei-shell-color-text-body"],
    ["--mei-color-text-inverse", "--mei-shell-color-text-inverse"],
    ["--mei-color-text-accent", "--mei-shell-color-text-accent"],
    ["--mei-color-surface-bg", "--mei-shell-color-panel-bg"],
    ["--mei-color-border-default", "--mei-shell-color-border-default"],
  ];
  for (const [from, to] of shellColorMap) {
    out = out.replaceAll(from, to);
  }
  for (let n = 1; n <= 5; n++) {
    out = out.replaceAll(`--mei-font-${n}`, `--mei-shell-font-${n}`);
  }
  out = out.replace(
    /font-family:\s*Inter[^;]+;/,
    "font-family: var(--mei-shell-family-ui);",
  );
  out = out.replace(
    /\.topbar-shell\s*\{[^}]*border-bottom:\s*1px solid[^;]+;/s,
    (block) =>
      block.replace(
        /border-bottom:\s*1px solid[^;]+;/,
        "border-bottom: 1px solid var(--mei-chrome-border-top);",
      ),
  );
  out = out.replace(
    /\.statusbar-shell\s*\{[^}]*border-top:\s*1px solid[^;]+;/s,
    (block) =>
      block.replace(
        /border-top:\s*1px solid[^;]+;/,
        "border-top: 1px solid var(--mei-chrome-border-bottom);",
      ),
  );
  out = out.replace(/var\((--mei-shell-[^,)]+),\s*[^)]+\)/g, "var($1)");
  return out + rest;
}

const appShell = path.join(root, "app/assets/app-shell.css");
let css = fs.readFileSync(appShell, "utf8");
css = migrateShellSection(css, "/* page-flow");
fs.writeFileSync(appShell, css);
console.log("migrated shell chrome tokens in app-shell.css");

const hostShell = path.join(root, "app/assets/host-shell.css");
let host = fs.readFileSync(hostShell, "utf8");
host = host
  .replace(
    /background:[^;]+;/,
    "background: var(--mei-shell-bg);",
    1,
  )
  .replace(/color:\s*#e2e8f0;/g, "color: var(--mei-shell-text);")
  .replace(/color:\s*#64748b;/g, "color: var(--mei-shell-color-text-muted);")
  .replace(/color:\s*#fb7185;/g, "color: var(--mei-shell-color-accent);")
  .replace(/color:\s*#fda4af;/g, "color: var(--mei-shell-color-accent-muted);")
  .replace(
    /font-family:[^;]+;/,
    "font-family: var(--mei-shell-family-ui);",
  );
fs.writeFileSync(hostShell, host);
console.log("migrated host-shell.css");
