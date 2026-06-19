#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const ROOT = path.join(import.meta.dirname, "..", "stock", "components");

const QUOTED_HEX_TO_TOKEN = new Map([
  ["'#e2e8f0'", 'color("text_body")'],
  ['"#e2e8f0"', 'color("text_body")'],
  ["'#E2E8F0'", 'color("text_body")'],
  ['"#f8fafc"', 'color("text_inverse")'],
  ["'#f8fafc'", 'color("text_inverse")'],
  ['"#94a3b8"', 'color("text_muted")'],
  ["'#94a3b8'", 'color("text_muted")'],
  ['"#cbd5e1"', 'color("text_body")'],
  ["'#cbd5e1'", 'color("text_body")'],
  ['"#93c5fd"', 'color("text_unit")'],
  ['"#7dd3fc"', 'color("text_unit")'],
  ['"#e0f2fe"', 'color("text_highlight")'],
  ['"#bae6fd"', 'color("text_highlight")'],
  ['"#fecaca"', 'color("status_error")'],
  ['"#fca5a5"', 'color("status_error")'],
  ['"#fda4af"', 'color("status_error")'],
  ['"#dbeafe"', 'color("text_body")'],
]);

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "vendor") continue;
      walk(full, out);
    } else if (entry.name.endsWith(".js") && !entry.name.includes("theme-fallback")) {
      out.push(full);
    }
  }
  return out;
}

function ensureColorImport(src, file) {
  if (src.includes('from "../mei/theme-style.js"') || src.includes('from "./theme-style.js"')) {
    return src;
  }
  const rel =
    file.includes("/mei/") ? null : file.includes("/cockpit/")
      ? 'import { color } from "../mei/theme-style.js";\n'
      : 'import { color } from "../mei/theme-style.js";\n';
  if (!rel) return src;
  const idx = src.indexOf("\n");
  return src.slice(0, idx + 1) + rel + src.slice(idx + 1);
}

for (const file of walk(ROOT)) {
  let src = fs.readFileSync(file, "utf8");
  let changed = false;
  for (const [hex, repl] of QUOTED_HEX_TO_TOKEN) {
    if (src.includes(hex)) {
      src = src.split(hex).join(repl);
      changed = true;
    }
  }
  if (!changed) continue;
  src = ensureColorImport(src, file);
  fs.writeFileSync(file, src);
  console.log("inline-colors", path.relative(ROOT, file));
}
