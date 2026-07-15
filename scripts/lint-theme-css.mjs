#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const hexRe = /#[0-9a-fA-F]{3,8}\b/g;
const rgbaRe = /rgba?\([^)]+\)/g;
const pxFontRe = /font-size:\s*\d+px/g;
const fallbackRe = /var\(--mei-[^,)]+,\s*[^)]+\)/g;
const shellEndMarkers = ["/* shell chrome end", "/* page-flow"];

function lintCss(file, css) {
  const lines = css.split("\n");
  let failed = false;
  let inRoot = false;
  let rootDepth = 0;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (/^:root\s*\{/.test(line.trim())) {
      inRoot = true;
      rootDepth = 1;
      continue;
    }
    if (inRoot) {
      rootDepth += (line.match(/\{/g) || []).length;
      rootDepth -= (line.match(/\}/g) || []).length;
      if (rootDepth <= 0) inRoot = false;
      continue;
    }
    if (/--mei-shell-color-literal_/.test(line)) {
      console.error(`${file}:${i + 1}: forbidden shell literal_* var: ${line.trim()}`);
      failed = true;
    }
    for (const re of [hexRe, rgbaRe, pxFontRe, fallbackRe]) {
      re.lastIndex = 0;
      if (re.test(line)) {
        console.error(`${file}:${i + 1}: forbidden literal: ${line.trim()}`);
        failed = true;
      }
    }
  }
  return failed;
}

let failed = false;
const appShell = path.join(root, "host-shell/app/assets/app-shell.css");
const appRaw = fs.readFileSync(appShell, "utf8");
let shellEnd = appRaw.length;
for (const marker of shellEndMarkers) {
  const idx = appRaw.indexOf(marker);
  if (idx !== -1 && idx < shellEnd) shellEnd = idx;
}
const shellCss = shellEnd === appRaw.length ? appRaw : appRaw.slice(0, shellEnd);
if (lintCss(appShell, shellCss)) failed = true;

const hostShell = path.join(root, "host-shell/app/assets/host-shell.css");
if (lintCss(hostShell, fs.readFileSync(hostShell, "utf8"))) failed = true;

if (failed) process.exit(1);
console.log("theme CSS lint passed (shell chrome + host-shell)");
