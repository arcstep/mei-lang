#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const cssPath = path.resolve("app/assets/app-shell.css");
let css = fs.readFileSync(cssPath, "utf8");

const colorMap = [
  ["#f8fafc", "--mei-color-text-inverse"],
  ["#e2e8f0", "--mei-color-text-primary"],
  ["#cbd5e1", "--mei-color-text-body"],
  ["#94a3b8", "--mei-color-text-muted"],
  ["#dbe8f6", "--mei-shell-text"],
  ["#f87171", "--mei-status-chip-danger-text"],
  ["#fbbf24", "--mei-status-chip-warn-text"],
  ["#bfdbfe", "--mei-status-chip-info-text"],
];

const fontMap = [
  ["10px", "--mei-font-1"],
  ["11px", "--mei-font-1"],
  ["12px", "--mei-font-1"],
  ["13px", "--mei-font-2"],
  ["14px", "--mei-font-2"],
  ["15px", "--mei-font-3"],
  ["16px", "--mei-font-3"],
];

for (const [hex, token] of colorMap) {
  const re = new RegExp(`color:\\s*${hex.replace("#", "#")}\\b`, "gi");
  css = css.replace(re, `color: var(${token}, ${hex})`);
}

for (const [px, token] of fontMap) {
  const re = new RegExp(`font-size:\\s*${px.replace(".", "\\.")}\\b`, "g");
  css = css.replace(re, `font-size: var(${token}, ${px})`);
}

const rgbaColorProps = [
  "background",
  "background-color",
  "border",
  "border-color",
  "box-shadow",
];
for (const prop of rgbaColorProps) {
  const re = new RegExp(`(${prop}):\\s*(rgba?\\([^;]+\\))`, "gi");
  css = css.replace(re, (match, p, val) => {
    if (match.includes("var(--mei-")) return match;
    return `${p}: var(--mei-color-surface-bg, ${val})`;
  });
}

fs.writeFileSync(cssPath, css);
console.log("migrated theme css tokens in app-shell.css");
