#!/usr/bin/env node
/**
 * Verify SPA behaviors on a running host (surface switch, tree, app tab).
 */
import { chromium } from "@playwright/test";

const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const appUrl = `${base}/apps/pretty-panels/view?surface=app`;

function fail(msg) {
  console.error(`FAIL: ${msg}`);
  process.exitCode = 1;
}

function pass(msg) {
  console.log(`PASS: ${msg}`);
}

async function main() {
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  const results = [];
  let documentFetches = 0;

  page.on("request", (req) => {
    if (req.isNavigationRequest() && req.frame() === page.mainFrame()) {
      documentFetches += 1;
    }
  });

  await page.goto(appUrl, { waitUntil: "domcontentloaded", timeout: 120000 });

  const bootCheck = await page.evaluate(() => ({
    navigateInternal: typeof window.__meiLangBoot?.navigateInternal === "function",
    navigateSpa: typeof window.__meiLangBoot?.navigateSpa === "function",
    syncTopbar: typeof window.__meiLangBoot?.syncTopbarActiveState === "function",
    syncAppTab: typeof window.__meiLangBoot?.syncAppTabActiveState === "function",
    navigateSurface: typeof window.__meiLangBoot?.navigateSurface === "function",
    forceRematerialize: String(window.__meiLangBoot?.viewRevisionClient?.tryAssembleLocal || "").includes(
      "forceRematerialize",
    ),
  }));
  console.log("boot exports:", bootCheck);
  if (!bootCheck.navigateInternal) fail("boot.navigateInternal missing on page");
  else pass("boot.navigateInternal present");

  documentFetches = 0;
  const beforeLayoutUrl = page.url();
  const layoutHref = await page.locator('sl-button[data-mei-app-view="布局"]').first().getAttribute("href");
  console.log("before layout:", { beforeLayoutUrl, layoutHref });
  const layoutBtn = page.locator('sl-button[data-mei-app-view="布局"]').first();
  await layoutBtn.evaluate((el) => el.click());
  await page.waitForTimeout(1500);
  const layoutDocFetches = documentFetches;

  const afterLayout = await page.evaluate(() => ({
    href: location.href,
    surface: document.body.getAttribute("data-surface"),
    treeBound: !!document.querySelector(".build-reachability-tree")?.__buildTreeTabBound,
    activeView: Array.from(
      document.querySelectorAll("sl-button[data-mei-app-view]"),
    )
      .filter((b) => b.classList.contains("is-active"))
      .map((b) => b.getAttribute("data-mei-app-view")),
    fullNavCount: 0,
  }));
  console.log("after layout click:", { ...afterLayout, layoutDocFetches });
  if (layoutDocFetches > 0) fail(`surface switch caused ${layoutDocFetches} document fetch(es)`);
  else pass("surface switch: no full page reload");
  if (!String(afterLayout.href).includes("/apps/pretty-panels/")) fail(`surface switch left wrong app: ${afterLayout.href}`);
  else pass("surface switch: stayed on pretty-panels");
  if (!String(afterLayout.href).includes("surface=layout")) fail(`URL missing surface=layout: ${afterLayout.href}`);
  else pass("surface switch: URL updated");
  if (afterLayout.surface !== "layout") fail(`body data-surface=${afterLayout.surface}`);
  else pass("surface switch: body data-surface=layout");
  if (!afterLayout.activeView.includes("布局")) fail(`topbar active=${JSON.stringify(afterLayout.activeView)}`);
  else pass("surface switch: topbar 布局 active");

  await page.waitForTimeout(800);
  documentFetches = 0;
  const treeLink = page.locator("a.build-tree-link, a.build-tree-label--link").first();
  await treeLink.waitFor({ state: "attached", timeout: 30000 });
  const treeCount = await treeLink.count();
  if (treeCount === 0) {
    console.warn("SKIP: no build tree links on layout surface");
  } else {
    const beforeTreeUrl = page.url();
    await treeLink.evaluate((el) => el.click());
    await page.waitForTimeout(1200);
    const afterTree = await page.evaluate(() => location.href);
    const treeDocFetches = documentFetches;
    console.log("tree click:", { beforeTreeUrl, afterTree, treeDocFetches });
    if (treeDocFetches > 0) fail(`tree click caused ${treeDocFetches} document fetch(es)`);
    else pass("tree click: no full page reload");
    if (String(afterTree).includes("/view?") && afterTree.includes("node=")) {
      pass("tree click: unified view node selection");
    } else if (String(afterTree).includes("/layout?") || String(afterTree).includes("/prototype?")) {
      fail(`tree navigated to legacy workspace URL: ${afterTree}`);
    } else {
      fail(`tree click unexpected URL: ${afterTree}`);
    }

    const secondTreeLink = page.locator("a.build-tree-link").nth(2);
    if ((await secondTreeLink.count()) > 0) {
      await secondTreeLink.evaluate((el) => el.click());
      await page.waitForTimeout(1200);
      const nodeSwitch = await page.evaluate(() => {
        const nodeParam = new URL(location.href).searchParams.get("node") || "";
        const active = Array.from(
          document.querySelectorAll(".build-reachability-tree .build-tree-link--active, .build-reachability-tree .build-tree-label--link.build-tree-link--active"),
        ).map((el) => el.getAttribute("data-build-node"));
        return { nodeParam, active };
      });
      console.log("layout tree second click:", nodeSwitch);
      if (!nodeSwitch.nodeParam) fail("layout tree second click: URL missing node param");
      else pass("layout tree second click: URL node updated");
      if (!nodeSwitch.active.includes(nodeSwitch.nodeParam)) {
        fail(`layout tree active mismatch: url=${nodeSwitch.nodeParam} active=${JSON.stringify(nodeSwitch.active)}`);
      } else pass("layout tree second click: active matches URL");
    }
  }

  documentFetches = 0;
  const protoBtn = page.locator('sl-button[data-mei-app-view="原型"]').first();
  if ((await protoBtn.count()) === 0) {
    console.warn("SKIP: prototype topbar button not found");
  } else {
    await protoBtn.evaluate((el) => el.click());
    await page.waitForTimeout(2500);
    const afterProto = await page.evaluate(() => ({
      href: location.href,
      surface: document.body.getAttribute("data-surface"),
      firstLinkHref: document.querySelector("a.build-tree-link, a.build-tree-label--link")?.href || "",
    }));
    console.log("after prototype switch:", afterProto);
    if (!String(afterProto.href).includes("surface=prototype")) {
      fail(`prototype switch URL missing surface=prototype: ${afterProto.href}`);
    } else pass("prototype switch: URL updated");
    if (afterProto.surface !== "prototype") fail(`body data-surface=${afterProto.surface}`);
    else pass("prototype switch: body data-surface=prototype");
    if (!String(afterProto.firstLinkHref).includes("surface=prototype")) {
      fail(`prototype tree link missing surface=prototype: ${afterProto.firstLinkHref}`);
    } else pass("prototype switch: tree links use prototype surface");

    documentFetches = 0;
    const protoTreeLink = page.locator("a.build-tree-link, a.build-tree-label--link").first();
    await protoTreeLink.waitFor({ state: "attached", timeout: 30000 });
    const beforeProtoTree = page.url();
    await protoTreeLink.evaluate((el) => el.click());
    await page.waitForTimeout(1500);
    const afterProtoTree = await page.evaluate(() => ({
      href: location.href,
      nodeParam: new URL(location.href).searchParams.get("node") || "",
      surfaceParam: new URL(location.href).searchParams.get("surface") || "",
    }));
    console.log("prototype tree click:", { beforeProtoTree, ...afterProtoTree, protoDocFetches: documentFetches });
    if (documentFetches > 0) fail(`prototype tree click caused ${documentFetches} document fetch(es)`);
    else pass("prototype tree click: no full page reload");
    if (!afterProtoTree.nodeParam) fail("prototype tree click: URL missing node param");
    else pass("prototype tree click: node selected");
    if (afterProtoTree.surfaceParam !== "prototype") {
      fail(`prototype tree click lost surface=prototype: ${afterProtoTree.href}`);
    } else pass("prototype tree click: stayed on prototype surface");
  }

  documentFetches = 0;
  await page.goto(appUrl, { waitUntil: "load", timeout: 120000 });
  const miniParkTab = page.locator('a.app-tab[data-app-id="mini-park"], a.app-tab[href*="mini-park"]').first();
  await miniParkTab.waitFor({ state: "visible", timeout: 30000 }).catch(() => {});
  const tabCount = await page.locator('a.app-tab[data-app-id="mini-park"], a.app-tab[href*="mini-park"]').count();
  if (tabCount === 0) {
    console.warn("SKIP: mini-park app tab not found");
  } else {
    documentFetches = 0;
    await page.locator('sl-button[data-mei-app-view="应用"]').first().evaluate((el) => el.click());
    await page.waitForTimeout(1000);
    documentFetches = 0;
    await miniParkTab.click({ timeout: 30000 });
    await page.waitForTimeout(2000);
    const afterApp = await page.evaluate(() => ({
      href: location.href,
      activeApps: Array.from(document.querySelectorAll("a.app-tab, a.app-tab-sub"))
        .filter((a) => a.classList.contains("active"))
        .map((a) => {
          try {
            return new URL(a.href, location.href).pathname.split("/")[2];
          } catch (_) {
            return a.textContent?.trim();
          }
        }),
    }));
    const appDocFetches = documentFetches;
    console.log("app switch:", { ...afterApp, appDocFetches });
    if (appDocFetches > 0) fail(`app switch caused ${appDocFetches} document fetch(es)`);
    else pass("app switch: no full page reload");
    if (!String(afterApp.href).includes("/apps/mini-park/")) fail(`URL not mini-park: ${afterApp.href}`);
    else pass("app switch: URL is mini-park");
    if (!afterApp.activeApps.includes("mini-park")) fail(`active app tabs=${JSON.stringify(afterApp.activeApps)}`);
    else pass("app switch: mini-park tab active");
  }

  await browser.close();
  if (!process.exitCode) {
    console.log("\nAll automated SPA checks passed.");
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
