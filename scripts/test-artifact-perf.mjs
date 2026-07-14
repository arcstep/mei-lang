#!/usr/bin/env node

/**
 * 0515 artifact perf gate — requires running host at MEI_SERVER_URL.
 */
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const serverUrl = (process.env.MEI_SERVER_URL || "http://127.0.0.1:9527").replace(/\/+$/, "");

async function fetchManifest(appId, scene) {
  const url = `${serverUrl}/api/host/scene-manifest?app_id=${encodeURIComponent(appId)}&scene=${encodeURIComponent(scene)}`;
  const started = Date.now();
  const res = await fetch(url);
  const manifest_fetch_ms = Date.now() - started;
  const structure_hit = res.headers.get("x-mei-structure-hit") === "1";
  const eval_hit = res.headers.get("x-mei-eval-hit") === "1";
  return { status: res.status, structure_hit, eval_hit, manifest_fetch_ms };
}

async function main() {
  const cold = await fetchManifest("zhifa", "home");
  const warm = await fetchManifest("zhifa", "home");
  const report = { cold, warm, serverUrl };
  console.log(JSON.stringify(report, null, 2));
  if (cold.status !== 200) {
    process.exit(1);
  }
  if (!warm.structure_hit) {
    console.error("expected warm structure_hit");
    process.exit(1);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
