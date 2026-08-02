#!/usr/bin/env node
/**
 * 0524 §3 board acceptance + E3 (--ui) + E4 (neighbor bootstrap).
 * Usage: node scripts/audit/eval-board-audit.mjs [baseUrl] [--ui]
 */
import { chromium } from "@playwright/test";
import { resolveAppId } from "../lib/resolve-app.mjs";

const argv = process.argv.slice(2);
const uiMode = argv.includes("--ui");
const appId = resolveAppId({ argv });
const base = (argv.find((a) => !a.startsWith("--")) || "http://127.0.0.1:9527").replace(
  /\/+$/,
  "",
);
const appUrl = `${base}/apps/${appId}/app`;

function isEvalPackHttp(url) {
  const u = new URL(url);
  const p = u.pathname;
  if (p.includes("/api/host/scene-bootstrap")) return true;
  if (p.includes("/api/host/view-revision")) return true;
  if (p.includes("/api/host/scene-manifest")) return true;
  if (p.includes("/api/host/layer-batch")) return true;
  return false;
}

function isEvalRuntimeApi(url) {
  const u = new URL(url);
  const p = u.pathname;
  if (p.includes("/api/datasets/metrics/")) return true;
  if (p.includes("/api/datasets/") && p.includes("/query")) return true;
  return false;
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const packCalls = [];
  const runtimeCalls = [];

  page.on("request", (req) => {
    const url = req.url();
    if (isEvalPackHttp(url)) {
      packCalls.push({ method: req.method(), url });
    }
    if (isEvalRuntimeApi(url)) {
      runtimeCalls.push({ method: req.method(), url });
    }
  });

  await page.goto(appUrl, { waitUntil: "networkidle", timeout: 120000 });
  await page.waitForTimeout(3500);

  const clientStateBefore = await page.evaluate(() => ({
    bootstrapSeeded: !!window.__meiBootstrapSeeded,
    evalPackSource: String(window.__meiEvalPackSource || ""),
    bootstrapScope: String(window.__mei?.bootstrap_scope || ""),
    neighborScopes: Array.isArray(window.__mei?.bootstrap_scopes)
      ? window.__mei.bootstrap_scopes.length
      : 0,
    neighborScopeIds: Array.isArray(window.__mei?.bootstrap_scopes)
      ? window.__mei.bootstrap_scopes.map((e) =>
          String(e?.bootstrapScope || e?.bootstrap_scope || "").trim(),
        )
      : [],
  }));

  const beforeActivate = { pack: packCalls.length, runtime: runtimeCalls.length };
  let activated = "";

  if (uiMode) {
    const clickTarget = await page.evaluate(() => {
      const el =
        document.querySelector('[data-mei-drilldown-active="true"]') ||
        document.querySelector("[data-mei-drilldown-metric]") ||
        document.querySelector("[data-mei-drilldown-scene]");
      if (!el) return { ok: false, reason: "no_drilldown_target" };
      const rect = el.getBoundingClientRect();
      return {
        ok: true,
        tag: el.tagName,
        scene: el.getAttribute("data-mei-drilldown-scene") || "",
        metric: el.getAttribute("data-mei-drilldown-metric") || "",
        x: rect.x + rect.width / 2,
        y: rect.y + rect.height / 2,
      };
    });
    if (!clickTarget.ok) {
      activated = `ui-click-failed:${clickTarget.reason}`;
    } else {
      await page.mouse.click(clickTarget.x, clickTarget.y);
      await page.waitForTimeout(3500);
      const overlayOpen = await page.evaluate(() => {
        const overlay = document.querySelector(
          "#mei-access-drilldown-overlay, #mei-access-scene-board-overlay",
        );
        return !!overlay && overlay.getAttribute("aria-hidden") !== "true";
      });
      activated = overlayOpen
        ? `ui-click:${clickTarget.scene || clickTarget.metric || clickTarget.tag}`
        : "ui-click:overlay_not_open";
    }
  } else {
    activated = await page.evaluate((targetAppId) => {
      const boot = window.__meiLangBoot || {};
      const homeScope = String(window.__mei?.bootstrap_scope || "home").trim();
      const neighbor = Array.isArray(window.__mei?.bootstrap_scopes)
        ? window.__mei.bootstrap_scopes
            .map((entry) =>
              String(entry?.bootstrapScope || entry?.bootstrap_scope || "").trim(),
            )
            .find((scope) => scope && scope !== homeScope)
        : "";
      const sceneId = neighbor || "penalty_total_analytics_page";
      if (typeof boot.dispatchScopeActivation === "function") {
        return boot.dispatchScopeActivation({
          appId: targetAppId,
          sceneId,
          scope: sceneId,
          source: "eval-board-audit",
        })
          ? `dispatch:${sceneId}`
          : "dispatch-failed";
      }
      window.dispatchEvent(
        new CustomEvent("meilang:scope-activation", {
          detail: {
            appId: targetAppId,
            sceneId,
            scope: sceneId,
            source: "eval-board-audit",
          },
        }),
      );
      return `event:${sceneId}`;
    }, appId);
    await page.waitForTimeout(3000);
  }

  const activationPack = packCalls.length - beforeActivate.pack;
  const activationRuntime = runtimeCalls.length - beforeActivate.runtime;
  const evalHttp = activationPack + activationRuntime;

  const clientState = await page.evaluate(() => ({
    bootstrapSeeded: !!window.__meiBootstrapSeeded,
    evalPackSource: String(window.__meiEvalPackSource || ""),
    bootstrapScope: String(window.__mei?.bootstrap_scope || ""),
    neighborScopes: Array.isArray(window.__mei?.bootstrap_scopes)
      ? window.__mei.bootstrap_scopes.length
      : 0,
  }));

  await browser.close();

  const failures = [];
  if (!activated || activated === "dispatch-failed" || activated.startsWith("ui-click-failed")) {
    failures.push(`scope activation failed: ${activated}`);
  }
  if (uiMode && activated === "ui-click:overlay_not_open") {
    failures.push("overlay did not open after drilldown click");
  }
  if (evalHttp > 1) {
    failures.push(`expected <=1 eval HTTP on scope activation, got ${evalHttp}`);
  }
  if (activationRuntime > 0) {
    failures.push(`expected 0 plug-ds runtime API on scope activation, got ${activationRuntime}`);
  }
  if (!uiMode && activationPack > 0) {
    failures.push(
      `expected 0 pack HTTP when neighbor prefetched (E4), got pack=${activationPack}`,
    );
  }
  if (clientStateBefore.neighborScopes < 2 && activationPack > 0) {
    failures.push(
      `expected bootstrap_scopes.length >= 2 when pack HTTP occurs, got ${clientStateBefore.neighborScopes}`,
    );
  }

  const report = {
    ok: failures.length === 0,
    mode: uiMode ? "ui" : "synthetic",
    url: appUrl,
    activated,
    activation: {
      evalHttp,
      packHttp: activationPack,
      runtimeApi: activationRuntime,
    },
    clientStateBefore,
    clientState,
    failures,
  };

  console.log(JSON.stringify(report, null, 2));
  if (failures.length > 0) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
