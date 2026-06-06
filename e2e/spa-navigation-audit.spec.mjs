/**
 * SPA 场景审计：只报告当前行为，不假定旧用例已通过即代表用户路径正常。
 * 需本机已运行: cargo run -p mei-lang-server -- serve  (默认 3000)
 *
 *   MEI_TEST_BASE_URL=http://127.0.0.1:3000 MEI_TEST_SKIP_SERVER=1 npx playwright test e2e/spa-navigation-audit.spec.mjs
 */
import { test, expect } from "@playwright/test";

const EXAMPLES_MAIN =
  "/apps/build/examples/core/02-external-scene-file?file=main.mei&tab=preview";
const EXAMPLES_HOME =
  "/apps/build/examples/core/02-external-scene-file?file=home.mei&tab=preview";
const SPBJW_HOME =
  "/apps/build/spbjw?file=scenes/home.mei&tab=preview";

const SPA_HEADER = "x-mei-spa-nav";
const IDLE_MS = 30000;

async function openManage(page, path) {
  const res = await page.goto(path, {
    waitUntil: "domcontentloaded",
    timeout: 90000,
  });
  expect(res?.ok()).toBeTruthy();
  await page.waitForFunction(
    () =>
      window.__meiLangBoot?.spaNavigationMounted === true &&
      typeof window.__meiLangBoot?.navigateSpa === "function",
    { timeout: 60000 },
  );
}

function trackNavigationSignals(page) {
  const log = {
    spaFetchUrls: [],
    manageJsLoads: 0,
    fullDocumentLoads: 0,
    consoleErrors: [],
  };
  page.on("request", (req) => {
    if (req.method() !== "GET") return;
    const url = req.url();
    if (req.headers()[SPA_HEADER] === "1") log.spaFetchUrls.push(url);
    if (url.includes("/app-bundles/manage.js")) log.manageJsLoads += 1;
  });
  page.on("framenavigated", (frame) => {
    if (frame === page.mainFrame()) log.fullDocumentLoads += 1;
  });
  page.on("console", (msg) => {
    if (msg.type() === "error") log.consoleErrors.push(msg.text());
  });
  return log;
}

async function readUiState(page) {
  return page.evaluate(() => {
    const overlay = document.querySelector('[data-mei-manage-nav-loading="true"]');
    const global = document.getElementById("mei-spa-loading");
    return {
      href: location.href,
      manageNavOverlay: !!overlay,
      manageNavOverlayText: overlay?.textContent?.slice(0, 120) || "",
      globalSpaLoading: global?.classList.contains("is-visible") || false,
      inFlight: window.__meiLangBoot?._spaInFlight ?? null,
    };
  });
}

async function waitIdleOrRecord(page, timeoutMs = IDLE_MS) {
  const deadline = Date.now() + timeoutMs;
  let last = await readUiState(page);
  while (Date.now() < deadline) {
    const busy =
      last.manageNavOverlay ||
      last.globalSpaLoading ||
      (await page
        .locator("#workspace-root main.main")
        .getAttribute("aria-busy")
        .catch(() => null)) === "true";
    if (!busy) return { idle: true, last };
    await page.waitForTimeout(250);
    last = await readUiState(page);
  }
  return { idle: false, last };
}

/**
 * @param {import('@playwright/test').Page} page
 * @param {() => Promise<void>} action
 */
async function auditScenario(page, action) {
  const beforeManageJs = (
    await page.evaluate(() => performance.getEntriesByType("resource"))
  ).filter((e) => e.name.includes("/app-bundles/manage.js")).length;

  const log = trackNavigationSignals(page);
  const manageJsBefore = log.manageJsLoads;

  await action();

  const idleResult = await waitIdleOrRecord(page);
  const after = await readUiState(page);
  const manageJsAfter = log.manageJsLoads;

  return {
    spaFetchCount: log.spaFetchUrls.length,
    spaFetchUrls: log.spaFetchUrls.slice(0, 3),
    manageJsLoadsDuring: manageJsAfter - manageJsBefore,
    fullDocumentLoads: log.fullDocumentLoads,
    idle: idleResult.idle,
    ui: idleResult.last || after,
    consoleErrors: log.consoleErrors.filter((t) =>
      /spa-navigation|ReferenceError|navigate/.test(t),
    ),
    resourceManageJsCount: beforeManageJs,
  };
}

test.describe("SPA 审计（对齐用户操作路径）", () => {
  test.describe.configure({ mode: "serial" });

  test("A. examples：main.mei → 侧栏 home.mei（用户日志路径）", async ({
    page,
  }) => {
    await openManage(page, EXAMPLES_MAIN);
    const homeLink = page
      .locator('aside.sidebar.left a.tree-link[href*="file=home.mei"]')
      .first();
    await expect(homeLink).toBeVisible();

    const report = await auditScenario(page, async () => {
      await homeLink.click();
    });

    console.log("[audit A]", JSON.stringify(report, null, 2));

    expect(report.spaFetchCount, "应出现带 x-mei-spa-nav 的 fetch").toBeGreaterThan(0);
    expect(report.idle, "应在 30s 内结束「正在切换预览」遮罩").toBe(true);
    expect(report.ui.href, "URL 应含 home.mei").toMatch(/file=home\.mei/);
    expect(
      report.manageJsLoadsDuring,
      "SPA 换文件不应再拉 manage.js",
    ).toBe(0);
  });

  test("B. examples：侧栏 main.mei（与旧 E2E 相同链接）", async ({ page }) => {
    await openManage(page, EXAMPLES_HOME);
    const mainLink = page
      .locator('aside.sidebar.left a.tree-link[href*="file=main.mei"]')
      .first();
    await expect(mainLink).toBeVisible();

    const report = await auditScenario(page, async () => {
      await mainLink.click();
    });
    console.log("[audit B]", JSON.stringify(report, null, 2));

    expect(report.spaFetchCount).toBeGreaterThan(0);
    expect(report.idle).toBe(true);
    expect(report.ui.href).toMatch(/file=main\.mei/);
    expect(report.manageJsLoadsDuring).toBe(0);
  });

  test("C. examples：预览 ↔ 源码 Tab", async ({ page }) => {
    await openManage(page, EXAMPLES_HOME);
    const report = await auditScenario(page, async () => {
      await page.getByRole("tab", { name: "源码" }).click();
      await page.waitForTimeout(300);
      await page.getByRole("tab", { name: "预览" }).click();
    });
    console.log("[audit C]", JSON.stringify(report, null, 2));

    expect(report.idle).toBe(true);
    expect(report.ui.href).toMatch(/tab=preview/);
    expect(report.manageJsLoadsDuring).toBe(0);
  });

  test("D. spbjw：顶栏 模板库 → 子应用链接", async ({ page }) => {
    await openManage(page, SPBJW_HOME);
    const report = await auditScenario(page, async () => {
      await page.getByRole("button", { name: "模板库", exact: true }).click();
      await page
        .locator(".app-group-menu a[href*='/apps/build/templates']")
        .first()
        .click();
    });
    console.log("[audit D]", JSON.stringify(report, null, 2));

    expect(report.spaFetchCount).toBeGreaterThan(0);
    expect(report.idle).toBe(true);
    expect(report.ui.href).toMatch(/\/apps\/manage\/templates/);
  });

  test("E. spbjw：构建 → 应用 → 构建", async ({ page }) => {
    await openManage(page, SPBJW_HOME);
    const appBtn = page
      .locator('header.topbar-shell sl-button[href*="/apps/app/"]')
      .first();
    const buildBtn = page
      .locator('header.topbar-shell sl-button[href*="/apps/build/"]')
      .first();

    const report = await auditScenario(page, async () => {
      await appBtn.click();
      await page.waitForURL(/\/apps\/app\//, { timeout: 20000 });
      await waitIdleOrRecord(page, 20000);
      await buildBtn.click();
    });
    console.log("[audit E]", JSON.stringify(report, null, 2));

    expect(report.spaFetchCount).toBeGreaterThan(0);
    expect(report.idle).toBe(true);
    expect(report.ui.href).toMatch(/\/apps\/build\//);
  });

  test("F. API：navigateSpa(home) 后遮罩必须清除", async ({ page }) => {
    await openManage(page, EXAMPLES_MAIN);
    const report = await auditScenario(page, async () => {
      await page.evaluate(async () => {
        const u = new URL("?file=home.mei&tab=preview", location.href).href;
        await window.__meiLangBoot.navigateSpa(u, true);
      });
    });
    console.log("[audit F]", JSON.stringify(report, null, 2));

    expect(report.spaFetchCount).toBeGreaterThan(0);
    expect(report.idle).toBe(true);
    expect(report.ui.href).toMatch(/file=home\.mei/);
  });
});
