import { test, expect } from "@playwright/test";

const ACCESS_URL =
  "/apps/app/examples/slides/01-web-slides-baseline/scene/intro";
const PRESENTATION_URL =
  "/apps/presentation/examples/slides/01-web-slides-baseline/scene/intro";

async function openPresentation(page, url = PRESENTATION_URL) {
  const res = await page.goto(url, {
    waitUntil: "domcontentloaded",
    timeout: 90000,
  });
  expect(res?.ok()).toBeTruthy();
  await expect(page.locator("#presentation-shell")).toBeVisible({
    timeout: 20000,
  });
}

test.describe("presentation route smoke", () => {
  test("access 页顶栏演示按钮指向 presentation route", async ({ page }) => {
    const res = await page.goto(ACCESS_URL, {
      waitUntil: "domcontentloaded",
      timeout: 90000,
    });
    expect(res?.ok()).toBeTruthy();

    const launch = page.locator("sl-button.topbar-launch-btn").first();
    await expect(launch).toBeVisible({ timeout: 20000 });
    await expect(launch).toHaveAttribute(
      "href",
      "/apps/presentation/examples/slides/01-web-slides-baseline/scene/intro",
    );
  });

  test("presentation 页支持左右键翻页并在首尾保持边界", async ({ page }) => {
    await openPresentation(page);

    await expect(page).toHaveURL(/\/scene\/intro$/);
    await expect(page.locator("text=1 / 3")).toBeVisible();
    await expect(page.locator('a[rel="prev"]')).toHaveCount(0);
    await expect(page.locator('a[rel="next"]')).toHaveAttribute(
      "href",
      "/apps/presentation/examples/slides/01-web-slides-baseline/scene/signal_board",
    );

    await page.keyboard.press("ArrowRight");
    await expect(page).toHaveURL(/\/scene\/signal_board$/);
    await expect(page.locator("text=2 / 3")).toBeVisible();
    await expect(page.locator("text=Scene 2: Signal Board")).toBeVisible();

    await page.keyboard.press("ArrowRight");
    await expect(page).toHaveURL(/\/scene\/qa_summary$/);
    await expect(page.locator("text=3 / 3")).toBeVisible();
    await expect(page.locator('a[rel="next"]')).toHaveCount(0);

    await page.keyboard.press("ArrowRight");
    await page.waitForTimeout(300);
    await expect(page).toHaveURL(/\/scene\/qa_summary$/);

    await page.keyboard.press("ArrowLeft");
    await expect(page).toHaveURL(/\/scene\/signal_board$/);
    await expect(page.locator('a[href="/apps/app/examples/slides/01-web-slides-baseline/scene/signal_board"]')).toBeVisible();
  });
});
