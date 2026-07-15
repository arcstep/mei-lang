/**
 * 专查 SPA 导航后 loading 遮罩是否长时间不消失（用户报告路径）
 */
import { test, expect } from "@playwright/test";

const EXAMPLES_MAIN =
  "/apps/examples/core/02-external-scene-file/layout?file=main.mei&tab=preview";

async function overlaySnapshot(page) {
  return page.evaluate(() => {
    const o = document.querySelector('[data-mei-manage-nav-loading="true"]');
    return {
      href: location.href,
      overlay: !!o,
      overlayText: o?.textContent?.replace(/\s+/g, " ").trim() || "",
      inFlight: window.__meiLangBoot?._spaInFlight ?? null,
    };
  });
}

test("main→home：点击后 5/15/30s 遮罩与 inFlight 快照", async ({ page }) => {
  await page.goto(EXAMPLES_MAIN, {
    waitUntil: "domcontentloaded",
    timeout: 90000,
  });
  await page.waitForFunction(
    () => window.__meiLangBoot?.spaNavigationMounted === true,
    { timeout: 60000 },
  );
  const link = page
    .locator('aside.sidebar.left a.tree-link[href*="file=home.mei"]')
    .first();
  await link.click();

  const snapshots = {};
  for (const sec of [5, 15, 30, 60]) {
    await page.waitForTimeout(sec === 5 ? 5000 : sec === 15 ? 10000 : sec === 30 ? 15000 : 30000);
    snapshots[`t${sec}s`] = await overlaySnapshot(page);
  }

  console.log("[stuck-audit]", JSON.stringify(snapshots, null, 2));

  const t60 = snapshots.t60s;
  expect(t60.overlay, "60s 后 manage-nav loading 遮罩应已消失").toBe(false);
  expect(t60.href).toMatch(/file=home\.mei/);
});
