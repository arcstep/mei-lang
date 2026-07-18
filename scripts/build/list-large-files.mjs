#!/usr/bin/env node
/**
 * 体量扫描：Rust 与 app/assets 自有 JS（排除 vendor/dist/target）。
 * 用法: node scripts/list-large-files.mjs [--warn-lines N] [--fail-lines M]
 * 若存在文件行数 >= fail-lines 则 exit 1；>= warn-lines 则 stderr 警告（仍 exit 0）。
 */
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "../..");

const SKIP_DIR_NAMES = new Set([
  "target",
  "target-check",
  "target-serve-check",
  "node_modules",
  "dist",
  "vendor",
]);

function parseArg(name, def) {
  const idx = process.argv.indexOf(name);
  if (idx === -1 || idx + 1 >= process.argv.length) return def;
  const n = Number(process.argv[idx + 1]);
  return Number.isFinite(n) ? n : def;
}

const WARN_LINES = parseArg("--warn-lines", 800);
const FAIL_LINES = parseArg("--fail-lines", 5000);

async function walkFiles(dir, acc, relBase = "") {
  let entries;
  try {
    entries = await readdir(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const ent of entries) {
    const name = ent.name;
    if (SKIP_DIR_NAMES.has(name)) continue;
    const full = path.join(dir, name);
    const rel = path.join(relBase, name);
    if (ent.isDirectory()) {
      await walkFiles(full, acc, rel);
    } else if (ent.isFile()) {
      acc.push({ full, rel });
    }
  }
}

function lineCountFromBuffer(buf) {
  let n = 0;
  for (let i = 0; i < buf.length; i++) {
    if (buf[i] === 10) n++;
  }
  if (buf.length > 0 && buf[buf.length - 1] !== 10) n++;
  return n;
}

function isOwnedJs(rel) {
  if (!rel.startsWith(`app${path.sep}assets${path.sep}`)) return false;
  if (rel.includes(`${path.sep}dist${path.sep}`)) return false;
  if (rel.includes(`${path.sep}vendor${path.sep}`)) return false;
  return rel.endsWith(".js");
}

function isRust(rel) {
  return rel.endsWith(".rs") && !rel.includes(`${path.sep}target`);
}

async function main() {
  const all = [];
  await walkFiles(root, all);

  const rust = [];
  const js = [];
  for (const { full, rel } of all) {
    if (isRust(rel)) {
      rust.push(full);
    } else if (isOwnedJs(rel)) {
      js.push(full);
    }
  }

  async function rank(paths) {
    const rows = [];
    for (const full of paths) {
      const buf = await readFile(full);
      const lines = lineCountFromBuffer(buf);
      rows.push({ full, lines, rel: path.relative(root, full) });
    }
    rows.sort((a, b) => b.lines - a.lines);
    return rows;
  }

  const rustRank = await rank(rust);
  const jsRank = await rank(js);

  console.log("=== mei-lang 大文件扫描（行数）===\n");
  console.log(`--warn-lines=${WARN_LINES} --fail-lines=${FAIL_LINES}\n`);

  console.log("--- Rust (top 25) ---");
  rustRank.slice(0, 25).forEach((r) => {
    console.log(String(r.lines).padStart(5) + "  " + r.rel);
  });

  console.log("\n--- app/assets 自有 JS (top 20) ---");
  jsRank.slice(0, 20).forEach((r) => {
    console.log(String(r.lines).padStart(5) + "  " + r.rel);
  });

  let maxLines = 0;
  let worst = "";
  for (const r of [...rustRank, ...jsRank]) {
    if (r.lines > maxLines) {
      maxLines = r.lines;
      worst = r.rel;
    }
  }

  let failed = false;
  for (const r of [...rustRank, ...jsRank]) {
    if (r.lines >= FAIL_LINES) {
      console.error(`\n[FAIL] ${r.rel} 行数 ${r.lines} >= ${FAIL_LINES}`);
      failed = true;
    } else if (r.lines >= WARN_LINES) {
      console.error(`\n[WARN] ${r.rel} 行数 ${r.lines} >= ${WARN_LINES}`);
    }
  }

  console.log(`\n最大: ${maxLines} 行 (${worst})`);
  if (failed) process.exit(1);
}

await main();
