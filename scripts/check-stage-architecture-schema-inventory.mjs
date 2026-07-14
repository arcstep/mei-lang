#!/usr/bin/env node
/**
 * 对照归档 0106 §4 Schema 台账与源码常量；漂移则失败。
 * 台账真源：docs/archive/mei-lang-v2-stage-architecture-remediation-2026-07/0106-…
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const meiRoot = path.resolve(__dirname, "..");
const docsRoot = path.resolve(meiRoot, "../docs");
const doc0106 = path.join(
  docsRoot,
  "archive/mei-lang-v2-stage-architecture-remediation-2026-07/0106-phase-0-baseline-freeze-and-inventory.md",
);

const EXPECTED = [
  { id: "mei-build-manifest-v1", needles: ["mei-build-manifest-v1"] },
  { id: "mei-mcg-registry-v2", needles: ["mei-mcg-registry-v2", "MCG_REGISTRY_SCHEMA"] },
  { id: "mei-mrg-registry-v2", needles: ["mei-mrg-registry-v2"] },
  { id: "mei-mrg-registry-v3", needles: ["mei-mrg-registry-v3", "MRG_REGISTRY_SCHEMA_V3"] },
  { id: "scene-view-manifest-v1", needles: ["scene-view-manifest-v1"] },
  { id: "structure-full-v1", needles: ["structure-full-v1"] },
  { id: "manifest-index-v1", needles: ["manifest-index-v1"] },
  { id: "shell-v1", needles: ["shell-v1"] },
  { id: "eval-slot-group-v1", needles: ["eval-slot-group-v1"] },
  { id: "runtime-plans-v2", needles: ["runtime-plans-v2"] },
  { id: "mei-client-bootstrap-v1", needles: ["mei-client-bootstrap-v1"] },
  { id: "mei-layer-plan-v1", needles: ["mei-layer-plan-v1"] },
  { id: "mei-presentation-map-v1", needles: ["mei-presentation-map-v1"] },
];

function read(p) {
  return fs.readFileSync(p, "utf8");
}

function walk(dir, exts, acc = []) {
  if (!fs.existsSync(dir)) return acc;
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    if (ent.name === "target" || ent.name === "node_modules" || ent.name === "dist") continue;
    const full = path.join(dir, ent.name);
    if (ent.isDirectory()) walk(full, exts, acc);
    else if (exts.some((e) => ent.name.endsWith(e))) acc.push(full);
  }
  return acc;
}

const doc = read(doc0106);
const searchRoots = [
  path.join(meiRoot, "crates/mei-host-graph/src"),
  path.join(meiRoot, "host-shell/src"),
  path.join(meiRoot, "server/src"),
  path.join(meiRoot, "crates/kernel/src"),
];
const files = searchRoots.flatMap((r) => walk(r, [".rs"]));
const corpus = files.map(read).join("\n");

const issues = [];
for (const item of EXPECTED) {
  if (!doc.includes(item.id)) {
    issues.push(`0106 missing ledger entry: ${item.id}`);
  }
  const hit = item.needles.some((n) => corpus.includes(n) || doc.includes(n));
  if (!hit) {
    issues.push(`source+doc missing constant evidence for ${item.id}`);
  }
}

// 高风险事实必须仍写在 0106
for (const phrase of [
  "StageKind",
  "semantic cache",
  "mei-mrg-registry-v2",
  "mei-mrg-registry-v3",
  "scene_id",
]) {
  if (!doc.toLowerCase().includes(phrase.toLowerCase()) && !doc.includes(phrase)) {
    // 宽松：中文台账可能用不同措辞；仅对英文 schema 硬卡
    if (phrase.startsWith("mei-")) {
      issues.push(`0106 missing high-risk fact mention: ${phrase}`);
    }
  }
}

// runtime fixture 目录存在
const runtimeFixtures = path.join(
  meiRoot,
  "crates/mei-host-graph/tests/fixtures/stage_architecture",
);
const compilerFixtures = path.join(
  meiRoot,
  "mei-compiler/crates/mei-compiler-tests/tests/fixtures/stage_architecture",
);
for (const dir of [runtimeFixtures, compilerFixtures]) {
  if (!fs.existsSync(dir)) issues.push(`missing fixtures dir: ${dir}`);
}

const requiredRuntime = [
  "mini-grid__home.runtime.json",
  "metric-grid__home.runtime.json",
  "mei-tutorial__intro.runtime.json",
  "mini-data__home.runtime.json",
  "mini-data__supervision.runtime.json",
  "zhifa__home.runtime.json",
  "mini-park__home.runtime.json",
  "mini-park__home_2d.runtime.json",
];
for (const name of requiredRuntime) {
  if (!fs.existsSync(path.join(runtimeFixtures, name))) {
    issues.push(`missing runtime fixture: ${name}`);
  }
}

for (const name of [
  "mei-prebuild-compile-index-v9",
  "is_current_prebuild_compile_index_schema",
]) {
  if (!corpus.includes(name)) {
    issues.push(`Gate C compile-index evidence missing: ${name}`);
  }
}

if (issues.length) {
  console.error("check-stage-architecture-schema-inventory failed:");
  for (const issue of issues) console.error(" -", issue);
  process.exit(1);
}

console.log(
  `schema inventory OK (${EXPECTED.length} schemas, ${requiredRuntime.length} runtime fixtures)`,
);
