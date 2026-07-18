#!/usr/bin/env node
/**
 * End-to-end timing audit: wall clock vs network vs client assembly/render.
 *
 * Usage:
 *   node scripts/e2e-timing-audit.mjs [baseUrl]
 *   node scripts/e2e-timing-audit.mjs http://127.0.0.1:9527 --json /tmp/e2e-timing.json
 *
 * Mirrors real UX paths: F5 refresh, surface switch, repeat switch.
 * Correlates Playwright request timestamps, Resource Timing, server headers,
 * MeiVisitHistoryStore load sessions, and __meiCacheDiag events.
 */
import fs from "node:fs/promises";
import { chromium } from "@playwright/test";

const args = process.argv.slice(2);
const base = (args.find((a) => !a.startsWith("-")) || "http://127.0.0.1:9527").replace(
  /\/+$/,
  "",
);
const jsonOut = (() => {
  const i = args.indexOf("--json");
  return i >= 0 ? args[i + 1] : "";
})();
const appUrl = `${base}/apps/zhifa/view`;

const HOST_API = [
  { kind: "document", test: (u) => /\/view(\?|$)/.test(u.pathname) && !u.pathname.includes("/api/") },
  { kind: "view-revision", test: (u) => u.pathname.includes("/api/host/view-revision") },
  { kind: "layer-batch", test: (u) => u.pathname.includes("/api/host/layer-batch") },
  { kind: "scene-manifest", test: (u) => u.pathname.includes("/api/host/scene-manifest") },
  { kind: "scene-bootstrap", test: (u) => u.pathname.includes("/api/host/scene-bootstrap") },
];

function classifyUrl(urlStr) {
  let u;
  try {
    u = new URL(urlStr);
  } catch {
    return "other";
  }
  for (const rule of HOST_API) {
    if (rule.test(u)) return rule.kind;
  }
  if (u.pathname.startsWith("/api/host/")) return "host-other";
  return "other";
}

function ms(n) {
  const v = Number(n);
  return Number.isFinite(v) ? Math.round(v) : null;
}

function summarizeRequests(requests) {
  const byKind = {};
  let firstStart = Infinity;
  let lastEnd = 0;
  let networkWall = 0;
  for (const r of requests) {
    byKind[r.kind] = (byKind[r.kind] || 0) + 1;
    if (r.startedAt < firstStart) firstStart = r.startedAt;
    if (r.endedAt > lastEnd) lastEnd = r.endedAt;
    networkWall += r.durationMs || 0;
  }
  const spanMs =
    firstStart < Infinity && lastEnd > firstStart ? Math.round(lastEnd - firstStart) : 0;
  return {
    count: requests.length,
    byKind,
    spanMs,
    serialSumMs: Math.round(networkWall),
    items: requests,
  };
}

function buildBreakdown(wallMs, network, client) {
  const span = network.spanMs || 0;
  const afterNetwork = Math.max(0, wallMs - span);
  const handlerReady = ms(client?.handlerReadyMs) || 0;
  const renderMs = ms(client?.renderMs) || 0;
  const evalMs = ms(client?.evalMs) || 0;
  const sessionTotal = ms(client?.totalMs) || 0;
  return {
    wall_total_ms: wallMs,
    network_span_ms: span,
    network_serial_sum_ms: network.serialSumMs,
    client_after_last_response_ms: afterNetwork,
    ssr_handler_ready_ms: handlerReady,
    client_session_render_ms: renderMs,
    client_session_eval_ms: evalMs,
    client_session_total_ms: sessionTotal,
    interpretation:
      span > 0
        ? `网络往返占 ${span}ms（并行跨度），末包后客户端仍耗约 ${afterNetwork}ms`
        : `无 host API 记录，${wallMs}ms 主要为客户端或 document 内联阶段`,
  };
}

async function waitSurfaceReady(page, surface, timeoutMs = 60000) {
  const deadline = Date.now() + timeoutMs;
  await page.waitForURL(new RegExp(`surface=${surface}`), { timeout: timeoutMs }).catch(() => {});
  while (Date.now() < deadline) {
    const state = await page.evaluate((targetSurface) => {
      const boot = window.__meiLangBoot || {};
      const overlay = document.getElementById("mei-spa-loading");
      const loadingVisible = Boolean(overlay?.classList?.contains("is-visible"));
      const ctx = boot.parseViewContext?.(location.href) || {};
      const cur = String(ctx.surface || ctx.mode || "app").toLowerCase();
      const appPanel = document.getElementById("mei-surface-app");
      const wsPanel = document.getElementById("mei-surface-workspace");
      const panel = targetSurface === "app" ? appPanel : wsPanel;
      const panelVisible = panel && !panel.hidden;
      const previewRoot =
        targetSurface === "app"
          ? appPanel?.querySelector?.("[data-preview-scope], [data-mei-frame-viewport]")
          : wsPanel?.querySelector?.(
              "[data-preview-scope], [data-mei-frame-viewport], .build-tree-node",
            );
      const hasContent = Boolean(previewRoot);
      return {
        surface: cur,
        loadingVisible,
        panelVisible,
        hasContent,
        outcome: boot.lastViewRevisionOutcome || null,
      };
    }, surface);
    if (
      state.surface === surface &&
      !state.loadingVisible &&
      state.panelVisible &&
      state.hasContent
    ) {
      return state;
    }
    await page.waitForTimeout(150);
  }
  return page.evaluate((targetSurface) => ({
    surface: targetSurface,
    loadingVisible: true,
    timedOut: true,
    outcome: window.__meiLangBoot?.lastViewRevisionOutcome || null,
  }), surface);
}

async function collectClientSnapshot(page) {
  return page.evaluate(() => {
    const boot = window.__meiLangBoot || {};
    const ctx = boot.parseViewContext?.(location.href) || {};
    const list = window.MeiVisitHistoryStore?.list?.() || [];
    const record = Array.isArray(list) && list.length ? list[0] : null;
    const active = boot.getActiveLoadSession?.() || null;
    const session = record || (active && !active.finalized ? active : null);
    const nav = performance.getEntriesByType("navigation")[0];
    const resources = performance
      .getEntriesByType("resource")
      .filter((e) => /\/api\/host\/|\/apps\/.*\/view/.test(String(e.name)))
      .map((e) => ({
        name: e.name.split("?")[0].slice(-80),
        durationMs: Math.round(e.duration),
        ttfbMs: Math.round(e.responseStart - e.startTime),
        transferMs: Math.round(e.responseEnd - e.responseStart),
      }));
    const body = document.body;
    const pipeline = window.__meiRenderPipeline?.last || {};
    return {
      url: location.href,
      surface: ctx.surface || ctx.mode || "app",
      handlerReadyMs: Number(body?.dataset?.meiHandlerHtmlReadyMs) || null,
      visit: session
        ? {
            kind: session.kind,
            label: session.label,
            renderMs: session.renderMs ?? session.phases?.render?.durationMs,
            evalMs: session.evalMs,
            totalMs: session.totalMs,
            handlerReadyMs: session.handlerReadyMs,
            apiTotal: session.apiTotal,
            apiBytes: session.apiBytes,
            apiByKind: session.apiByKind || {},
            outcome: session.outcome,
            readyReason: session.readyReason,
          }
        : null,
      navigation: nav
        ? {
            domContentLoadedMs: Math.round(nav.domContentLoadedEventEnd),
            loadEventMs: Math.round(nav.loadEventEnd),
            responseEndMs: Math.round(nav.responseEnd),
          }
        : null,
      resourceTiming: resources.slice(-12),
      cacheDiag: (window.__meiCacheDiag?.events || []).slice(-16).map((e) => ({
        event: e.event,
        detail: e.detail,
      })),
      lastOutcome: boot.lastViewRevisionOutcome || null,
      digests: boot.readClientDigests?.(ctx) || {},
      pipeline: {
        userVisibleReadyMs: Number(pipeline.wallMs) || null,
        documentMs: Number(pipeline.documentMs) || null,
        layerRestoreMs: Number(pipeline.phases?.layer_restore?.durationMs) || 0,
        composeStructureMs: Number(pipeline.phases?.compose_structure?.durationMs) || 0,
        bindEvalSlotsMs: Number(pipeline.phases?.bind_eval_slots?.durationMs) || 0,
      },
    };
  });
}

function createRequestTracker(phaseStartMs) {
  const inflight = new Map();
  const finished = [];
  const onRequest = (req) => {
    const kind = classifyUrl(req.url());
    if (kind === "other" || kind === "host-other") return;
    inflight.set(req, { kind, url: req.url(), method: req.method(), startedAt: Date.now() });
  };
  const onResponse = async (res) => {
    const req = res.request();
    const st = inflight.get(req);
    if (!st) return;
    inflight.delete(req);
    const endedAt = Date.now();
    let statusHdr = "";
    let serverMs = "";
    try {
      const h = res.headers();
      statusHdr =
        h["x-mei-view-revision-status"] ||
        h["x-mei-scene-revision-status"] ||
        h["x-mei-handler-html-ready-ms"] ||
        "";
      serverMs = h["x-mei-handler-html-ready-ms"] || h["x-mei-total-ms"] || "";
    } catch (_) {}
    finished.push({
      kind: st.kind,
      method: st.method,
      status: res.status(),
      serverStatus: statusHdr,
      serverMs: serverMs ? Number(serverMs) : null,
      bytes: Number(res.headers()["content-length"]) || null,
      startedAt: st.startedAt - phaseStartMs,
      endedAt: endedAt - phaseStartMs,
      durationMs: endedAt - st.startedAt,
      url: st.url.slice(0, 180),
    });
  };
  return { onRequest, onResponse, finished };
}

async function runPhase(page, label, action, options = {}) {
  const phaseStart = Date.now();
  const tracker = createRequestTracker(phaseStart);
  page.on("request", tracker.onRequest);
  page.on("response", tracker.onResponse);
  await action();
  const ready = options.surface
    ? await waitSurfaceReady(page, options.surface, options.timeoutMs || 120000)
    : await page.waitForTimeout(500);
  const wallMs = Date.now() - phaseStart;
  page.off("request", tracker.onRequest);
  page.off("response", tracker.onResponse);
  const network = summarizeRequests(tracker.finished);
  const client = await collectClientSnapshot(page);
  const breakdown = buildBreakdown(wallMs, network, client.visit || client);
  return { label, ready, wallMs, network, client, breakdown };
}

async function clickSurface(page, surface) {
  const label = { app: "应用", layout: "布局", prototype: "原型" }[surface];
  const btn = page.locator(`sl-button[data-mei-app-view]:has-text("${label}")`).first();
  await btn.click({ timeout: 30000 });
}

function printPhase(phase) {
  const b = phase.breakdown;
  console.log(`\n${"=".repeat(72)}`);
  console.log(`${phase.label}  →  用户感知 wall=${b.wall_total_ms}ms`);
  console.log(b.interpretation);
  console.log(
    `  网络跨度=${b.network_span_ms}ms  串行合计=${b.network_serial_sum_ms}ms  末包后客户端≈${b.client_after_last_response_ms}ms`,
  );
  if (b.ssr_handler_ready_ms) {
    console.log(`  SSR handler就绪(服务端标头)=${b.ssr_handler_ready_ms}ms`);
  }
  if (phase.client.visit) {
    const v = phase.client.visit;
    console.log(
      `  客户端会话: render=${v.renderMs ?? "—"}ms eval=${v.evalMs ?? "—"}ms total=${v.totalMs ?? "—"}ms outcome=${v.outcome || phase.client.lastOutcome || "—"}`,
    );
  }
  console.log(`  ready: ${JSON.stringify(phase.ready)}`);
  console.log(`  请求(${phase.network.count}):`, JSON.stringify(phase.network.byKind));
  for (const r of phase.network.items) {
    const extra = r.serverStatus ? ` status=${r.serverStatus}` : "";
    const srv = r.serverMs ? ` srv=${r.serverMs}ms` : "";
    console.log(
      `    +${r.startedAt}ms ${r.kind} ${r.method} ${r.durationMs}ms${extra}${srv} ${r.bytes ?? "?"}B`,
    );
  }
  if (phase.client.resourceTiming?.length) {
    console.log("  ResourceTiming(末几条):");
    for (const rt of phase.client.resourceTiming.slice(-6)) {
      console.log(`    ${rt.name} dur=${rt.durationMs}ms ttfb=${rt.ttfbMs}ms xfer=${rt.transferMs}ms`);
    }
  }
  if (phase.client.cacheDiag?.length) {
    console.log("  cache-diag:", phase.client.cacheDiag.map((e) => e.event).join(" → "));
  }
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  await context.addInitScript(() => {
    try {
      localStorage.setItem("mei:cache-diag", "1");
    } catch (_) {}
  });
  const page = await context.newPage();

  const report = { base, measuredAt: new Date().toISOString(), phases: [] };

  report.phases.push(
    await runPhase(
      page,
      "1_F5_cold_app",
      async () => {
        await page.goto(`${appUrl}?surface=app`, { waitUntil: "domcontentloaded", timeout: 120000 });
      },
      { surface: "app" },
    ),
  );

  report.phases.push(
    await runPhase(
      page,
      "2_switch_layout_1st",
      async () => clickSurface(page, "layout"),
      { surface: "layout" },
    ),
  );

  report.phases.push(
    await runPhase(
      page,
      "3_switch_layout_2nd",
      async () => {
        await clickSurface(page, "prototype");
        await clickSurface(page, "layout");
      },
      { surface: "layout" },
    ),
  );

  report.phases.push(
    await runPhase(
      page,
      "4_F5_layout",
      async () => page.reload({ waitUntil: "domcontentloaded", timeout: 120000 }),
      { surface: "layout" },
    ),
  );

  report.phases.push(
    await runPhase(
      page,
      "5_switch_prototype_1st_after_f5",
      async () => clickSurface(page, "prototype"),
      { surface: "prototype" },
    ),
  );

  report.phases.push(
    await runPhase(
      page,
      "6_switch_app",
      async () => clickSurface(page, "app"),
      { surface: "app" },
    ),
  );

  await browser.close();

  const coldStartApps = [
    { label: "zhifa", path: "/apps/zhifa/view?surface=app&scene=home" },
    { label: "mini-park", path: "/apps/mini-park/view?surface=app&scene=home" },
  ];
  report.coldStartBenchmarks = [];
  {
    const benchBrowser = await chromium.launch({ headless: true });
    const benchPage = await benchBrowser.newPage();
    for (const entry of coldStartApps) {
      const started = Date.now();
      await benchPage.goto(`${base}${entry.path}`, {
        waitUntil: "networkidle",
        timeout: 120000,
      });
      const client = await benchPage.evaluate(() => {
        const marks = window.__meiRenderPipeline?.last?.marks || [];
        const coldEnd = marks.find((m) => m?.name === "cold_start:end");
        const previewEnd = marks.find((m) => m?.name === "preview_compose:end");
        return {
          coldStartEndMs: coldEnd?.detail?.wallMs ?? coldEnd?.detail?.ms ?? null,
          previewComposeEndMs: previewEnd?.detail?.wallMs ?? previewEnd?.detail?.ms ?? null,
          materialized:
            document
              .querySelector("#mei-compose-root, .preview-pane-scroll, .shell")
              ?.getAttribute("data-mei-compose-materialized") === "1",
        };
      });
      report.coldStartBenchmarks.push({
        app: entry.label,
        wallMs: Date.now() - started,
        ...client,
        targetMs: 1500,
      });
    }
    await benchBrowser.close();
  }

  report.zhifaWarmF5 = [];
  {
    const warmBrowser = await chromium.launch({ headless: true });
    const warmPage = await warmBrowser.newPage();
    await warmPage.goto(`${base}/apps/zhifa/home`, {
      waitUntil: "networkidle",
      timeout: 120000,
    });
    for (let run = 1; run <= 10; run += 1) {
      await warmPage.reload({ waitUntil: "domcontentloaded", timeout: 120000 });
      await warmPage.waitForFunction(
        () => window.__meiRenderPipeline?.last?.endedAt === "user_visible_ready",
        { timeout: 120000 },
      );
      report.zhifaWarmF5.push({
        run,
        ...(await collectClientSnapshot(warmPage)).pipeline,
      });
    }
    await warmBrowser.close();
  }

  for (const phase of report.phases) {
    printPhase(phase);
  }

  console.log(`\n${"=".repeat(72)}`);
  console.log("汇总（wall_total_ms = 用户感知端到端）");
  console.log("label | wall | net_span | after_net | requests");
  for (const p of report.phases) {
    const b = p.breakdown;
    console.log(
      `${p.label.padEnd(32)} | ${String(b.wall_total_ms).padStart(5)}ms | ${String(b.network_span_ms).padStart(5)}ms | ${String(b.client_after_last_response_ms).padStart(5)}ms | ${JSON.stringify(p.network.byKind)}`,
    );
  }
  if (report.coldStartBenchmarks?.length) {
    console.log("\n冷启动墙钟（compose-only，目标 <1.5s，见 0525）");
    for (const row of report.coldStartBenchmarks) {
      const cold = row.coldStartEndMs ?? row.wallMs;
      console.log(
        `${row.app.padEnd(12)} | wall=${String(row.wallMs).padStart(5)}ms | cold_start:end=${cold != null ? `${cold}ms` : "n/a"} | materialized=${row.materialized}`,
      );
    }
  }
  if (report.zhifaWarmF5.length) {
    const sorted = report.zhifaWarmF5
      .map((row) => row.userVisibleReadyMs)
      .filter(Number.isFinite)
      .sort((a, b) => a - b);
    const quantile = (p) => sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)];
    console.log(
      `\nzhifa 暖 F5: p50=${quantile(0.5)}ms p95=${quantile(0.95)}ms（预算 500/800ms）`,
    );
  }

  if (jsonOut) {
    await fs.writeFile(jsonOut, JSON.stringify(report, null, 2), "utf8");
    console.log(`\nJSON → ${jsonOut}`);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
