#!/usr/bin/env node
/**
 * 024005 过滤三层合同静态检查：
 * 1) JS：禁止 drilldown_filters 与 default_filters 双写同一赋值块
 * 2) .mei：同一 params 对象内禁止同维同时出现在 scope_filters 与 default_filters
 *
 * 用法：node ./scripts/check/lint-filter-layer-contract.mjs [root...]
 * 默认同仓 mei-lang + 可选 sibling workspaces（缺则跳过）。
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const meiLangRoot = path.resolve(__dirname, "../..");
const defaultRoots = [meiLangRoot];
const siblingWs = path.resolve(meiLangRoot, "../workspaces");
if (fs.existsSync(siblingWs)) defaultRoots.push(siblingWs);

const roots = process.argv.slice(2).map((p) => path.resolve(p));
const scanRoots = roots.length > 0 ? roots : defaultRoots;

const SKIP_DIR = new Set([
  "node_modules",
  ".git",
  "target",
  "dist",
  "assets/dist",
  "env",
  "upload",
]);

const errors = [];

function walk(dir, out = []) {
  let entries;
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return out;
  }
  for (const entry of entries) {
    if (SKIP_DIR.has(entry.name)) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(full, out);
      continue;
    }
    if (!entry.isFile()) continue;
    if (/\.(js|mei)$/i.test(entry.name)) out.push(full);
  }
  return out;
}

function lintJsDualWrite(file, text) {
  // detail.drilldown_filters = x; detail.default_filters = x; 同块
  const patterns = [
    /drilldown_filters\s*[:=]\s*([A-Za-z_$][\w$]*)\s*[,;][\s\S]{0,120}?default_filters\s*[:=]\s*\1\b/g,
    /drilldown_filters\s*:\s*([A-Za-z_$][\w$.]*)\s*,\s*default_filters\s*:\s*\1\b/g,
  ];
  for (const re of patterns) {
    let m;
    while ((m = re.exec(text))) {
      const line = text.slice(0, m.index).split("\n").length;
      errors.push(`${file}:${line}: drilldown_filters 与 default_filters 双写（024005 禁止）`);
    }
  }
}

function extractMapBody(text, key) {
  const re = new RegExp(`"${key}"\\s*:\\s*\\{([^{}]*)\\}`, "g");
  const maps = [];
  let m;
  while ((m = re.exec(text))) {
    maps.push({ body: m[1], index: m.index });
  }
  return maps;
}

function keysInMeiMapBody(body) {
  const keys = new Set();
  const re = /"([^"]+)"\s*:/g;
  let m;
  while ((m = re.exec(body))) keys.add(m[1]);
  return keys;
}

function lintMeiOverlap(file, text) {
  // 粗粒度：同文件内若同时出现两个 map，检查键交集（同 params 邻近块）
  const scopes = extractMapBody(text, "scope_filters");
  const seeds = extractMapBody(text, "default_filters");
  if (!scopes.length || !seeds.length) return;
  for (const scope of scopes) {
    const scopeKeys = keysInMeiMapBody(scope.body);
    for (const seed of seeds) {
      // 仅当两个 map 在 800 字符内视为同一 params
      if (Math.abs(scope.index - seed.index) > 800) continue;
      const seedKeys = keysInMeiMapBody(seed.body);
      for (const key of scopeKeys) {
        if (seedKeys.has(key)) {
          const line = text.slice(0, Math.min(scope.index, seed.index)).split("\n").length;
          errors.push(
            `${file}:${line}: 同维 "${key}" 同时出现在 scope_filters 与 default_filters（024005 禁止）`,
          );
        }
      }
    }
  }
}

for (const root of scanRoots) {
  if (!fs.existsSync(root)) {
    console.log(`[lint-filter-layer] skip missing root: ${root}`);
    continue;
  }
  for (const file of walk(root)) {
    const text = fs.readFileSync(file, "utf8");
    if (file.endsWith(".js")) lintJsDualWrite(file, text);
    if (file.endsWith(".mei")) lintMeiOverlap(file, text);
  }
}

if (errors.length) {
  console.error(`[lint-filter-layer] FAILED (${errors.length})`);
  for (const err of errors) console.error(`  ${err}`);
  process.exit(1);
}
console.log(`[lint-filter-layer] ok roots=${scanRoots.join(", ")}`);
