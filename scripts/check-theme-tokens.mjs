#!/usr/bin/env node
/**
 * Enforce theme token consumption in app-shell.css and core mei/dataset components.
 */
import fs from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..");

const SCAN_DIRS = [
  path.join(ROOT, "stock/components/mei"),
  path.join(ROOT, "stock/components/dataset"),
  path.join(ROOT, "host-shell/app/assets/app-shell.css"),
];

const ALLOWLIST = [
  /theme-fallback\.js$/,
  /theme-style\.js$/,
  /text\.js$/,
  /runtime-query\.js$/,
  /floating-text-popover\.js$/,
  /filter-bar\.js$/,
  /summary-cards\.js$/,
  /table\.js$/,
  /cells\.js$/,
];

const HEX = /#[0-9a-fA-F]{3,8}\b/g;
const RGBA = /rgba?\([^)]+\)/g;

function allowed(file) {
  return ALLOWLIST.some((re) => re.test(file));
}

function walk(dir, out = []) {
  if (!fs.existsSync(dir)) {
    return out;
  }
  if (fs.statSync(dir).isFile()) {
    out.push(dir);
    return out;
  }
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    walk(path.join(dir, entry.name), out);
  }
  return out;
}

function stripVars(text) {
  return text.replace(/var\([^)]*\)/g, "");
}

function scanFile(file) {
  if (allowed(file)) {
    return [];
  }
  const rel = path.relative(ROOT, file);
  const text = fs.readFileSync(file, "utf8");
  const issues = [];

  if (file.endsWith(".js")) {
    const stripped = stripVars(text);
    const hex = stripped.match(HEX) ?? [];
    const rgba = stripped.match(RGBA) ?? [];
    if (hex.length || rgba.length) {
      issues.push(
        `${rel}: hardcoded color (${[...hex, ...rgba].slice(0, 4).join(", ")})`,
      );
    }
  }

  if (file.endsWith(".css")) {
    const withoutRoot = text.replace(/:root\s*\{[\s\S]*?\}/m, "");
    const stripped = stripVars(withoutRoot);
    const colorHits = stripped.match(/color:\s*(#[0-9a-fA-F]{3,8}|rgba?\()/gi) ?? [];
    const fontHits = stripped.match(/font-size:\s*\d+px\b/g) ?? [];
    if (colorHits.length) {
      issues.push(`${rel}: ${colorHits.length} literal color rules`);
    }
    if (fontHits.length) {
      issues.push(`${rel}: ${fontHits.length} literal font-size rules`);
    }
  }

  return issues;
}

const files = SCAN_DIRS.flatMap((entry) => walk(entry));
const allIssues = files.flatMap(scanFile);

if (allIssues.length) {
  console.error("check-theme-tokens failed:\n" + allIssues.join("\n"));
  process.exit(1);
}

console.log(`check-theme-tokens passed (${files.length} files)`);
