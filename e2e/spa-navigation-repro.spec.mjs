/**
 * 复现 harness：任一检查失败即 test.fail，用于「先抓现象、不修代码」。
 * 运行（需 3000 上已有 serve）：
 *   MEI_TEST_BASE_URL=http://127.0.0.1:3000 MEI_TEST_SKIP_SERVER=1 npx playwright test e2e/spa-navigation-repro.spec.mjs
 */
import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

const BASE = process.env.MEI_TEST_BASE_URL || "http://127.0.0.1:3000";
const REPORT = path.join(process.cwd(), "e2e-reports", "spa-repro-last.json");

const SCENARIOS = [
  {
    id: "examples_sidebar_main_to_home",
    label: "examples 侧栏 main.mei → home.mei（用户日志路径）",
    open: `${BASE}/apps/build/examples/core/02-external-scene-file?file=main.mei&tab=preview`,
    action: async (page) => {
      await page
        .locator('aside.sidebar.left a.tree-link[href*="file=home.mei"]')
        .first()
        .click();
    },
    expectUrl: /file=home\.mei/,
  },
  {
    id: "examples_sidebar_home_to_main",
    label: "examples 侧栏 home.mei → main.mei",
    open: `${BASE}/apps/build/examples/core/02-external-scene-file?file=home.mei&tab=preview`,
    action: async (page) => {
      await page
        .locator('aside.sidebar.left a.tree-link[href*="file=main.mei"]')
        .first()
        .click();
    },
    expectUrl: /file=main\.mei/,
  },
  {
    id: "spbjw_sidebar_home_to_main",
    label: "spbjw 侧栏 home.mei → main.mei",
    open: `${BASE}/apps/build/spbjw?file=scenes/home.mei&tab=preview`,
    action: async (page) => {
      await page.getByRole("link", { name: "main.mei" }).click();
    },
    expectUrl: /file=main\.mei/,
  },
  {
    id: "examples_double_click_home",
    label: "examples 侧栏 home 连续双击（导航竞态）",
    open: `${BASE}/apps/build/examples/core/02-external-scene-file?file=main.mei&tab=preview`,
    action: async (page) => {
      const link = page
        .locator('aside.sidebar.left a.tree-link[href*="file=home.mei"]')
        .first();
      await link.dblclick();
    },
    expectUrl: /file=home\.mei/,
  },
  {
    id: "examples_click_before_spa_mounted",
    label: "examples 在 SPA 挂载前点击 home（模拟抢跑整页导航）",
    open: `${BASE}/apps/build/examples/core/02-external-scene-file?file=main.mei&tab=preview`,
    waitUntil: "commit",
    skipMountWait: true,
    action: async (page) => {
      await page
        .locator('aside.sidebar.left a.tree-link[href*="file=home.mei"]')
        .first()
        .click({ noWaitAfter: true });
    },
    expectUrl: /file=home\.mei/,
  },
  {
    id: "spbjw_topbar_templates",
    label: "spbjw 顶栏 模板库 → cockpit",
    open: `${BASE}/apps/build/spbjw?file=scenes/home.mei&tab=preview`,
    action: async (page) => {
      await page.getByRole("button", { name: "模板库", exact: true }).click();
      await page
        .locator(".app-group-menu a[href*='/apps/build/templates']")
        .first()
        .click();
    },
    expectUrl: /\/apps\/manage\/templates/,
  },
];

  const OVERLAY_WAIT_MS = 45000;
  const STUCK_INFLIGHT_MS = 5000;
const SPA_HEADER = "x-mei-spa-nav";

async function runScenario(page, scenario, waitUntilDefault) {
  const waitUntil = scenario.waitUntil || waitUntilDefault;
  const timeline = [];
  const log = (phase, data) => {
    timeline.push({ t: Date.now(), phase, ...data });
  };

  let manageJsAfterOpen = 0;
  let spaFetches = [];
  let fullDocNavigations = 0;
  let managePageWithoutSpa = [];

  const onReq = (req) => {
    if (req.method() !== "GET") return;
    const url = req.url();
    if (url.includes("/app-bundles/manage.js")) manageJsAfterOpen += 1;
    if (req.headers()[SPA_HEADER] === "1") spaFetches.push(url);
    if (url.includes("/apps/build/") && !url.includes("/app-")) {
      if (!req.headers()[SPA_HEADER]) managePageWithoutSpa.push(url);
    }
  };
  page.on("request", onReq);
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) fullDocNavigations += 1;
  });

  const t0 = Date.now();
  await page.goto(scenario.open, { waitUntil, timeout: 120000 });
  log("opened", { url: page.url() });

  let mounted = false;
  if (!scenario.skipMountWait) {
    mounted = await page
      .waitForFunction(
        () =>
          window.__meiLangBoot?.spaNavigationMounted === true &&
          typeof window.__meiLangBoot?.navigateSpa === "function",
        { timeout: 60000 },
      )
      .then(() => true)
      .catch(() => false);
    log("spa_mounted", { mounted });
  } else {
    log("spa_mounted", { mounted: false, skipped: true });
  }
  const manageJsBaseline = manageJsAfterOpen;
  spaFetches = [];
  managePageWithoutSpa = [];
  const docNavBaseline = fullDocNavigations;

  await scenario.action(page);
  log("clicked", {});

  let overlayStuck = false;
  let lastSnap = null;
  const deadline = Date.now() + OVERLAY_WAIT_MS;
  while (Date.now() < deadline) {
    lastSnap = await page.evaluate(() => {
      const o = document.querySelector('[data-mei-manage-nav-loading="true"]');
      const main = document.querySelector("#workspace-root main.main");
      return {
        href: location.href,
        overlay: !!o,
        overlayText: o?.textContent?.replace(/\s+/g, " ").trim().slice(0, 200) || "",
        ariaBusy: main?.getAttribute("aria-busy") === "true",
        inFlight: window.__meiLangBoot?._spaInFlight ?? null,
        navId: window.__meiLangBoot?._spaNavId ?? null,
      };
    });
    if (!lastSnap.overlay && !lastSnap.ariaBusy) break;
    await page.waitForTimeout(500);
  }
  if (lastSnap?.overlay || lastSnap?.ariaBusy) overlayStuck = true;

  await page.waitForTimeout(STUCK_INFLIGHT_MS);
  const inFlightStuck = await page.evaluate(() => {
    const o = document.querySelector('[data-mei-manage-nav-loading="true"]');
    return {
      inFlight: window.__meiLangBoot?._spaInFlight ?? 0,
      overlay: !!o,
      href: location.href,
    };
  });

  const consoleErrors = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") consoleErrors.push(msg.text());
  });

  page.off("request", onReq);

  const urlOk = scenario.expectUrl.test(page.url());
  const manageJsReload = manageJsAfterOpen > manageJsBaseline + 1;
  const docNavAfterClick = fullDocNavigations - docNavBaseline;
  const userLogPattern =
    managePageWithoutSpa.length >= 2 && manageJsReload && docNavAfterClick >= 1;

  return {
    id: scenario.id,
    label: scenario.label,
    waitUntil,
    durationMs: Date.now() - t0,
    mounted,
    url: page.url(),
    urlOk,
    overlayStuck,
    lastSnap,
    spaFetchCount: spaFetches.length,
    spaFetches: spaFetches.slice(0, 5),
    manageJsLoadsTotal: manageJsAfterOpen,
    manageJsReloadAfterClick: manageJsReload,
    fullDocNavigations,
    userLogPattern_fullManagePagePlusBundleReload: userLogPattern,
    managePageWithoutSpaCount: managePageWithoutSpa.length,
    docNavAfterClick,
    inFlightStuck,
    navigationIncomplete:
      !urlOk && (inFlightStuck.inFlight > 0 || inFlightStuck.overlay),
    reproduced:
      overlayStuck ||
      !urlOk ||
      (scenario.id.includes("sidebar") && spaFetches.length === 0) ||
      userLogPattern ||
      (!urlOk && inFlightStuck.inFlight > 0),
    timeline,
    consoleSpaErrors: consoleErrors.filter((t) =>
      /spa-navigation|ReferenceError|navigate/.test(t),
    ),
  };
}

test.describe("SPA 复现 harness", () => {
  test.describe.configure({ mode: "serial", timeout: 300000 });

  for (const waitUntil of ["domcontentloaded", "load"]) {
    test(`策略 waitUntil=${waitUntil}`, async ({ browser }) => {
      const context = await browser.newContext({
        baseURL: BASE,
        ignoreHTTPSErrors: true,
        viewport: { width: 1440, height: 900 },
      });
      await context.addInitScript(() => {
        window.__meiLangBoot = window.__meiLangBoot || {};
        let n = 0;
        const orig = window.__meiLangBoot.navigateSpa;
        if (typeof orig === "function") {
          window.__meiLangBoot.navigateSpa = function (url, rep) {
            n += 1;
            window.__meiLangBoot._spaNavCalls = n;
            return orig.call(this, url, rep);
          };
        }
      });
      await context.route("**/*", async (route) => {
        await route.continue();
      });

      const page = await context.newPage();
      const results = [];
      for (const sc of SCENARIOS) {
        results.push(await runScenario(page, sc, waitUntil));
      }
      await context.close();

      fs.mkdirSync(path.dirname(REPORT), { recursive: true });
      fs.writeFileSync(REPORT, JSON.stringify({ at: new Date().toISOString(), results }, null, 2));

      const reproduced = results.filter((r) => r.reproduced);
      console.log("\n======== SPA REPRO REPORT ========");
      for (const r of results) {
        const mark = r.reproduced ? "REPRO" : "ok";
        console.log(
          `[${mark}] ${r.id}: overlayStuck=${r.overlayStuck} urlOk=${r.urlOk} spaFetch=${r.spaFetchCount} manageJsReload=${r.manageJsReloadAfterClick} userLogPattern=${r.userLogPattern_fullManagePagePlusBundleReload}`,
        );
        if (r.overlayStuck && r.lastSnap) {
          console.log(`       overlay: ${r.lastSnap.overlayText}`);
        }
      }
      console.log(`Report: ${REPORT}`);
      console.log("==================================\n");

      expect(
        reproduced.length,
        `期望至少复现 1 个用户描述现象；详见 ${REPORT}`,
      ).toBeGreaterThan(0);
    });
  }
});
