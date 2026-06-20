#!/usr/bin/env node
/** Repair truncated gradient literals in workspace-host-theme.fragment.json */
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const root = path.resolve(import.meta.dirname, "..");
const fragmentPath = path.join(root, "scripts/workspace-host-theme.fragment.json");

function balancedFrom(css, start) {
  let depth = 0;
  let i = start;
  for (; i < css.length; i++) {
    const ch = css[i];
    if (ch === "(") depth++;
    else if (ch === ")") {
      depth--;
      if (depth === 0) return css.slice(start, i + 1);
    }
  }
  return css.slice(start);
}

function extractLiterals(css) {
  const out = new Set();
  for (let i = 0; i < css.length; i++) {
    const rest = css.slice(i);
    if (rest.startsWith("linear-gradient(") || rest.startsWith("radial-gradient(")) {
      const end = balancedFrom(css, i + rest.indexOf("("));
      out.add(end.startsWith("(") ? `linear-gradient${end}` : css.slice(i, i + end.length + (rest.startsWith("linear") ? rest.indexOf("(") + 1 : 0)));
      // simpler: find gradient keyword
    }
    if (rest.startsWith("rgba(") || rest.startsWith("rgb(")) {
      const p = i + rest.indexOf("(");
      out.add(balancedFrom(css, p));
    }
    if (/^#[0-9a-fA-F]{3,8}\b/.test(rest)) {
      const m = rest.match(/^#[0-9a-fA-F]{3,8}\b/);
      out.add(m[0]);
    }
  }
  return out;
}

function extractLiteralsSimple(css) {
  const out = new Set();
  const re =
    /linear-gradient\((?:[^()]+|\([^()]*\))+\)|radial-gradient\((?:[^()]+|\([^()]*\))+\)|rgba?\([^)]+\)|#[0-9a-fA-F]{3,8}\b/g;
  for (const m of css.matchAll(re)) out.add(m[0]);
  // second pass for nested parens in multi-stop gradients
  let pos = 0;
  while (pos < css.length) {
    const lg = css.indexOf("linear-gradient(", pos);
    if (lg === -1) break;
    const p = lg + "linear-gradient".length;
    out.add("linear-gradient" + balancedFrom(css, p));
    pos = lg + 1;
  }
  return out;
}

const orig = execSync("git show HEAD:app/assets/app-shell.css", {
  cwd: root,
  encoding: "utf8",
});
const shellEnd = orig.indexOf("/* page-flow");
const origShell = orig.slice(0, shellEnd);
const literals = extractLiteralsSimple(origShell);

const fragment = JSON.parse(fs.readFileSync(fragmentPath, "utf8"));
const color = fragment.themes.host.tokens.color;
let fixed = 0;
for (const [key, value] of Object.entries(color)) {
  if (typeof value !== "string") continue;
  const open = (value.match(/\(/g) || []).length;
  const close = (value.match(/\)/g) || []).length;
  if (open === close && open > 0) continue;
  if (open === 0 && close === 0) continue;
  // truncated: find full literal containing this prefix
  const candidate = [...literals].find(
    (lit) => lit.startsWith(value) || value.startsWith(lit.slice(0, 20)),
  );
  if (!candidate) {
    const byPrefix = [...literals].find((lit) => lit.includes(value.replace(/^\w+-gradient\(/, "").slice(0, 24)));
    if (byPrefix) {
      color[key] = byPrefix;
      fixed++;
      continue;
    }
    console.warn("unresolved", key, value.slice(0, 60));
    continue;
  }
  color[key] = candidate;
  fixed++;
}

fs.writeFileSync(fragmentPath, JSON.stringify(fragment, null, 2) + "\n");

const workspacePaths = [
  path.join(root, "../workspaces/ws-spbjw/.mei-workspace.json"),
  path.join(root, "../workspaces/ws-dev/.mei-workspace.json"),
  path.join(root, "../workspaces/ws-hello/.mei-workspace.json"),
];
for (const wsPath of workspacePaths) {
  if (!fs.existsSync(wsPath)) continue;
  const ws = JSON.parse(fs.readFileSync(wsPath, "utf8"));
  ws.ops.themes.host = fragment.themes.host;
  fs.writeFileSync(wsPath, JSON.stringify(ws, null, 2) + "\n");
}
console.log(`fixed ${fixed} truncated token values`);
