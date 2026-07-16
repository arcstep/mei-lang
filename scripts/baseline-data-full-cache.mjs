#!/usr/bin/env node
/**
 * P2.0 baseline matrix for zhifa + data-full.
 *
 * Runs on an independent Host port / generation so it does NOT clear the
 * user's currently running data-full service cache.
 *
 * Usage:
 *   node scripts/baseline-data-full-cache.mjs
 *   MEI_BASELINE_PORT=19627 MEI_BASELINE_ROUNDS=5 node scripts/baseline-data-full-cache.mjs
 *
 * Scenarios (each ≥ MEI_BASELINE_ROUNDS):
 *   - cold_generation
 *   - disk_hot_l1_cold
 *   - l1_hot
 *   - concurrent_same_key_{1,8,32}
 *
 * Targets: home shell, warning-count T2, enforcement-matters large rowset T2.
 */

import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const MEI_LANG_ROOT = path.resolve(__dirname, "..");
const WORKSPACE = path.resolve(
  MEI_LANG_ROOT,
  "../workspaces/ws-demo-v2"
);
const LAUNCH = path.join(
  WORKSPACE,
  "apps/zhifa/launch/data-full.json"
);
const PORT = Number(process.env.MEI_BASELINE_PORT || 19627);
const ROUNDS = Math.max(3, Number(process.env.MEI_BASELINE_ROUNDS || 5));
const OUT_DIR = path.resolve(
  process.env.MEI_BASELINE_OUT ||
    path.join(MEI_LANG_ROOT, "tmp/baseline-data-full")
);

function percentile(sorted, p) {
  if (!sorted.length) return null;
  const idx = Math.min(
    sorted.length - 1,
    Math.max(0, Math.ceil((p / 100) * sorted.length) - 1)
  );
  return sorted[idx];
}

function summarize(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  return {
    n: sorted.length,
    p50: percentile(sorted, 50),
    p95: percentile(sorted, 95),
    min: sorted[0] ?? null,
    max: sorted[sorted.length - 1] ?? null,
  };
}

async function sleep(ms) {
  await new Promise((r) => setTimeout(r, ms));
}

async function fetchJson(url, init) {
  const started = Date.now();
  const res = await fetch(url, init);
  const text = await res.text();
  let body = null;
  try {
    body = JSON.parse(text);
  } catch {
    body = { raw: text.slice(0, 500) };
  }
  return {
    ok: res.ok,
    status: res.status,
    ms: Date.now() - started,
    body,
  };
}

async function waitReady(base, timeoutMs = 180_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const r = await fetchJson(`${base}/api/host/runtime-snapshot`);
      if (r.ok) return r.body;
    } catch {
      // retry
    }
    await sleep(1000);
  }
  throw new Error(`host not ready within ${timeoutMs}ms on ${base}`);
}

function startHost() {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  const logPath = path.join(OUT_DIR, "host.log");
  const logFd = fs.openSync(logPath, "w");
  const child = spawn(
    path.join(WORKSPACE, "deploy/start.sh"),
    [
      "--cargo",
      "--app-config",
      "apps/zhifa/launch/data-full.json",
      "--port",
      String(PORT),
    ],
    {
      cwd: WORKSPACE,
      env: {
        ...process.env,
        MEI_HOST_PORT: String(PORT),
        MEI_BASELINE_MODE: "1",
      },
      stdio: ["ignore", logFd, logFd],
    }
  );
  return { child, logPath };
}

async function measurePage(base, pathName) {
  return fetchJson(`${base}${pathName}`);
}

async function measureConcurrent(base, pathName, concurrency) {
  const started = Date.now();
  const results = await Promise.all(
    Array.from({ length: concurrency }, () => measurePage(base, pathName))
  );
  return {
    wallMs: Date.now() - started,
    results,
    leaders: results.filter(
      (r) => r.body?.perf?.eval_singleflight_leader === 1
    ).length,
    waiters: results.filter(
      (r) => r.body?.perf?.eval_singleflight_waiter === 1
    ).length,
    persists: results.filter((r) => r.body?.perf?.eval_persist === 1).length,
  };
}

async function main() {
  // Soft-skip when sibling workspace is absent (standalone mei-lang clone).
  if (!fs.existsSync(WORKSPACE) || !fs.existsSync(LAUNCH)) {
    console.log(
      JSON.stringify(
        {
          skip: true,
          reason: "workspace or launch missing",
          workspace: WORKSPACE,
          launch: LAUNCH,
        },
        null,
        2
      )
    );
    process.exit(0);
  }
  console.log(
    JSON.stringify(
      {
        workspace: WORKSPACE,
        launch: LAUNCH,
        port: PORT,
        rounds: ROUNDS,
        outDir: OUT_DIR,
      },
      null,
      2
    )
  );

  const { child, logPath } = startHost();
  const base = `http://127.0.0.1:${PORT}`;
  const report = {
    startedAt: new Date().toISOString(),
    port: PORT,
    logPath,
    scenarios: {},
  };

  try {
    const snapshot = await waitReady(base);
    report.warmupLastRun = snapshot?.warmupLastRun ?? null;

    const targets = [
      { id: "home", path: "/apps/zhifa/" },
      {
        id: "warning_t2",
        path: "/apps/zhifa/home/t2/r-right-rail/s-warning",
      },
      {
        id: "enforcement_matters_t2",
        path: "/apps/zhifa/home/t2/r-left-rail/s-enforcement",
      },
    ];

    for (const target of targets) {
      const cold = [];
      for (let i = 0; i < ROUNDS; i++) {
        cold.push((await measurePage(base, target.path)).ms);
      }
      const hot = [];
      for (let i = 0; i < ROUNDS; i++) {
        hot.push((await measurePage(base, target.path)).ms);
      }
      const conc = {};
      for (const n of [1, 8, 32]) {
        const sample = await measureConcurrent(base, target.path, n);
        conc[`c${n}`] = {
          wallMs: sample.wallMs,
          leaders: sample.leaders,
          waiters: sample.waiters,
          persists: sample.persists,
          statusOk: sample.results.every((r) => r.ok),
        };
      }
      report.scenarios[target.id] = {
        path: target.path,
        cold: summarize(cold),
        l1Hot: summarize(hot),
        concurrent: conc,
      };
    }

    report.finishedAt = new Date().toISOString();
    const outFile = path.join(OUT_DIR, "baseline-report.json");
    fs.writeFileSync(outFile, JSON.stringify(report, null, 2));
    console.log(`wrote ${outFile}`);
    console.log(JSON.stringify(report.scenarios, null, 2));
  } finally {
    child.kill("SIGTERM");
    await sleep(1500);
    try {
      child.kill("SIGKILL");
    } catch {
      // ignore
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
