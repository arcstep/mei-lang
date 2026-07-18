#!/usr/bin/env node
/** Lint workspace shell theme: no literal_* keys, snake_case names, keys referenced in shell CSS. */
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "../..");
const fragmentPath = path.join(root, "scripts/theme/workspace-host-theme.fragment.json");
const appShellPath = path.join(root, "host-shell/app/assets/app-shell.css");
const hostShellPath = path.join(root, "host-shell/app/assets/host-shell.css");

const keyRe = /^[a-z][a-z0-9_]*$/;
const hashKeyRe = /^[a-z]+_[0-9a-f]{8}$/;

function collectUsedKeys(...texts) {
  const used = new Set();
  for (const text of texts) {
    for (const m of text.matchAll(/--mei-shell-color-([\w-]+)/g)) {
      used.add(m[1].replace(/-/g, "_"));
    }
  }
  return used;
}

const fragment = JSON.parse(fs.readFileSync(fragmentPath, "utf8"));
const colors = fragment.themes?.host?.tokens?.color ?? {};
const used = collectUsedKeys(
  fs.readFileSync(appShellPath, "utf8"),
  fs.readFileSync(hostShellPath, "utf8"),
);

let failed = false;
for (const key of Object.keys(colors)) {
  if (key.startsWith("literal_")) {
    console.error(`forbidden literal_* key: tokens.color.${key}`);
    failed = true;
  }
  if (!keyRe.test(key)) {
    console.error(`invalid key name: tokens.color.${key}`);
    failed = true;
  }
  if (hashKeyRe.test(key)) {
    console.error(`hash-like key forbidden: tokens.color.${key}`);
    failed = true;
  }
  if (!used.has(key) && key !== "watermark") {
    console.error(`unreferenced color key: tokens.color.${key}`);
    failed = true;
  }
}

for (const key of used) {
  if (!Object.prototype.hasOwnProperty.call(colors, key)) {
    console.error(`CSS references missing token: tokens.color.${key}`);
    failed = true;
  }
}

if (failed) process.exit(1);
console.log(`shell theme lint passed (${Object.keys(colors).length} color keys)`);
