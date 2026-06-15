/**
 * SPA 导航四类场景 E2E（需已启动 mei-lang-server，默认 http://127.0.0.1:3000）
 */
import { test, expect } from "@playwright/test";

/** 与 playwright.config 的 baseURL 一致；勿写死 3000 */
const EXAMPLES_MANAGE =
  process.env.MEI_SPA_EXAMPLES_URL ||
  "/apps/build/examples/core/02-external-scene-file?file=home.mei&tab=preview";

const SPBJW_MANAGE =
  process.env.MEI_SPA_MANAGE_URL ||
  "/apps/build/spbjw?file=scenes/home.mei&tab=preview";

const SPBJW_EFFECTIVENESS_MANAGE =
  process.env.MEI_SPA_EFFECTIVENESS_URL ||
  "/apps/build/spbjw?file=scenes/08-监督成效.mei&tab=preview";

const SPA_HEADER = "x-mei-spa-nav";

async function waitSpaIdle(page, timeoutMs = 15000) {
  await expect
    .poll(
      async () => {
        return page.evaluate(() => {
          const global = document.getElementById("mei-spa-loading");
          const globalBusy =
            global && global.classList.contains("is-visible");
          const main = document.querySelector("#workspace-root main.main");
          const mainBusy = main && main.getAttribute("aria-busy") === "true";
          const overlay = document.querySelector(
            '[data-mei-manage-nav-loading="true"]',
          );
          return !globalBusy && !mainBusy && !overlay;
        });
      },
      { timeout: timeoutMs },
    )
    .toBe(true);
}

function isHostBundleReload(url) {
  return (
    url.includes("/app-bundles/styles.css") ||
    url.includes("/app-bundles/shoelace.js")
  );
}

function trackHostBundleLoads(page) {
  const counts = { manage: 0, access: 0, shared: 0 };
  const onReq = (req) => {
    if (req.method() !== "GET") return;
    const url = req.url();
    if (url.includes("/app-bundles/manage.js")) counts.manage += 1;
    else if (url.includes("/app-bundles/access.js")) counts.access += 1;
    else if (isHostBundleReload(url)) counts.shared += 1;
  };
  page.on("request", onReq);
  return {
    counts: () => ({ ...counts }),
    stop: () => page.off("request", onReq),
  };
}

async function expectSpaFetch(page, action, urlMatcher) {
  const spaReq = page.waitForRequest(
    (req) => {
      if (req.method() !== "GET") return false;
      if (req.headers()[SPA_HEADER] !== "1") return false;
      return urlMatcher(req.url());
    },
    { timeout: 20000 },
  );
  await action();
  const req = await spaReq;
  expect(req.headers()[SPA_HEADER]).toBe("1");
  return req;
}

async function openManage(page, url) {
  const res = await page.goto(url, {
    waitUntil: "domcontentloaded",
    timeout: 90000,
  });
  expect(res?.ok()).toBeTruthy();
  await expect(page.locator("#workspace-root")).toBeVisible();
  await page.waitForFunction(
    () =>
      window.__meiLangBoot?.spaNavigationMounted === true &&
      typeof window.__meiLangBoot?.navigateSpa === "function",
    { timeout: 60000 },
  );
}

test.describe("examples 管理页", () => {
  test.beforeEach(async ({ page }) => {
    await openManage(page, EXAMPLES_MANAGE);
  });

  test("navigateInternal 可调用并完成", async ({ page }) => {
    const err = await page.evaluate(async () => {
      try {
        await window.__meiLangBoot.navigateSpa(
          new URL("?file=main.mei&tab=preview", window.location.href).href,
          true,
        );
        return "";
      } catch (e) {
        return String(e);
      }
    });
    expect(err).toBe("");
    await waitSpaIdle(page);
  });

  test("侧栏资源树：SPA fetch + 无 manage.js 重载", async ({ page }) => {
    const tracker = trackHostBundleLoads(page);
    const before = tracker.counts();
    const mainLink = page
      .locator('aside.sidebar.left a.tree-link[href*="file=main.mei"]')
      .first();
    await expect(mainLink).toBeVisible();
    await expectSpaFetch(
      page,
      () => mainLink.click(),
      (url) => url.includes("file=main.mei"),
    );
    await waitSpaIdle(page);
    await expect(page).toHaveURL(/file=main\.mei/);
    const after = tracker.counts();
    expect(after.manage).toBe(before.manage);
    tracker.stop();
  });
});

const runSpbjwE2e = process.env.MEI_RUN_SPBJW_E2E === "1";

test.describe("spbjw 完整壳", () => {
  test.skip(!runSpbjwE2e, "set MEI_RUN_SPBJW_E2E=1 for customer workspace e2e");
  test.beforeEach(async ({ page }) => {
    await openManage(page, SPBJW_MANAGE);
  });

  test("顶栏应用 Tab：SPA fetch", async ({ page }) => {
    const groupTrigger = page.getByRole("button", { name: "模板库", exact: true });
    await expect(groupTrigger).toBeVisible({ timeout: 10000 });
    await groupTrigger.click();
    const appLink = page
      .locator(".app-group-menu a[href*='/apps/build/templates']")
      .first();
    await expect(appLink).toBeVisible();
    await expectSpaFetch(
      page,
      () => appLink.click(),
      (url) => url.includes("/apps/build/templates"),
    );
    await waitSpaIdle(page, 20000);
  });

  test("预览/源码 Tab：仅客户端切换", async ({ page }) => {
    const tracker = trackHostBundleLoads(page);
    const before = tracker.counts();
    const sourceTab = page.getByRole("tab", { name: "源码" });
    await expect(sourceTab).toBeVisible();
    await sourceTab.click();
    await expect(page).toHaveURL(/tab=source/, { timeout: 5000 });
    expect(tracker.counts().manage).toBe(before.manage);
    const previewTab = page.getByRole("tab", { name: "预览" });
    await previewTab.click();
    await expect(page).toHaveURL(/tab=preview/);
    expect(tracker.counts().manage).toBe(before.manage);
    tracker.stop();
  });

  test("顶栏 应用/构建：SPA + 允许首次 access.js", async ({ page }) => {
    const appBtn = page
      .locator('header.topbar-shell sl-button[href*="/apps/app/"]')
      .first();
    const buildBtn = page
      .locator('header.topbar-shell sl-button[href*="/apps/build/"]')
      .first();
    await expect(appBtn).toBeVisible();
    await expect(buildBtn).toBeVisible();
    const tracker = trackHostBundleLoads(page);
    const before = tracker.counts();
    await expectSpaFetch(
      page,
      () => appBtn.click(),
      (url) => url.includes("/apps/app/"),
    );
    await waitSpaIdle(page, 20000);
    await expect(page).toHaveURL(/\/apps\/app\//);
    let mid = tracker.counts();
    expect(mid.manage).toBe(before.manage);
    expect(mid.access).toBeLessThanOrEqual(before.access + 1);
    await expectSpaFetch(
      page,
      () => buildBtn.click(),
      (url) => url.includes("/apps/build/"),
    );
    await waitSpaIdle(page, 20000);
    await expect(page).toHaveURL(/\/apps\/build\//);
    mid = tracker.counts();
    expect(mid.manage).toBeLessThanOrEqual(before.manage + 1);
    tracker.stop();
  });

  test("侧栏换文件：SPA fetch", async ({ page }) => {
    const link = page.getByRole("link", { name: "main.mei" });
    await expect(link).toBeVisible();
    await expectSpaFetch(page, () => link.click(), (url) => url.includes("file=main.mei"));
    await waitSpaIdle(page, 20000);
    await expect(page).toHaveURL(/file=main\.mei/);
  });
});

test.describe("spbjw explain 弹层", () => {
  test.skip(!runSpbjwE2e, "set MEI_RUN_SPBJW_E2E=1 for customer workspace e2e");
  test("处理人数指标可打开 overlay 并展示 detail 明细", async ({ page }) => {
    await openManage(page, SPBJW_EFFECTIVENESS_MANAGE);
    const metricButton = page.getByRole("button", {
      name: /查看指标明细：effectiveness_handled_person_times/,
    });
    await expect(metricButton).toBeVisible({ timeout: 20000 });
    await metricButton.click();
    const overlay = page.locator("#mei-access-drilldown-overlay");
    await expect(overlay).toBeVisible({ timeout: 20000 });
    await expect(page.getByRole("dialog", { name: "指标下钻明细" })).toBeVisible({
      timeout: 20000,
    });
    await expect(
      overlay.getByRole("tab", { name: "问题处理结果（处理人数）" }),
    ).toBeVisible({ timeout: 20000 });
    await expect(
      overlay.getByRole("tab", { name: "问题处理结果（处理人数）" }),
    ).toHaveAttribute("aria-selected", "true");
  });
});
