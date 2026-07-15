/**
 * Phase 8.5 Gate: temporary Stage (`~/`) + single `launch.json` policy smoke.
 *
 * MEI_E2E_BASE_URL=http://127.0.0.1:9527 npx playwright test e2e/phase-8-5-scope-gate.spec.mjs
 */
import { test, expect } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.env.MEI_TEST_BASE_URL || "").replace(
  /\/+$/,
  "",
);
const appId = String(process.env.MEI_E2E_APP_ID || "mini-data").trim();

test.describe("Phase 8.5 temp-stage + single-launch gate", () => {
  test.skip(!base, "set MEI_E2E_BASE_URL (or MEI_TEST_BASE_URL) to run");

  test("non-launch.json start is rejected", async ({ request }) => {
    const res = await request.post(
      `${base}/api/host/apps/${encodeURIComponent(appId)}/start`,
      { data: { config: "scoped-rail" } },
    );
    expect(res.status()).toBe(400);
    const body = await res.json();
    expect(String(body.error || "")).toMatch(/single-launch|launch\.json|ephemeral/i);
  });

  test("runtime overlay reset endpoint exists", async ({ request }) => {
    const reset = await request.post(
      `${base}/api/host/apps/${encodeURIComponent(appId)}/runtime-overlay/reset`,
      { data: {} },
    );
    expect([200, 202]).toContain(reset.status());
    const get = await request.get(
      `${base}/api/host/apps/${encodeURIComponent(appId)}/runtime-overlay`,
    );
    expect(get.ok()).toBeTruthy();
    const body = await get.json();
    expect(body.overlay == null || body.overlay === null).toBeTruthy();
  });

  test("temporary Stage ~/ route is accepted or redirects", async ({ request }) => {
    const res = await request.get(
      `${base}/apps/${encodeURIComponent(appId)}/~/home/T1`,
      { maxRedirects: 0 },
    );
    // 200 thin shell, 302/307 starting/canonical, or 404 if structure not warm yet.
    expect([200, 302, 307, 404, 409]).toContain(res.status());
  });

  test("legacy deep scoped path redirects toward ~/ when resolvable", async ({ request }) => {
    const res = await request.get(
      `${base}/apps/${encodeURIComponent(appId)}/home/t1`,
      { maxRedirects: 0 },
    );
    expect([200, 302, 307, 404, 409]).toContain(res.status());
    if (res.status() === 302 || res.status() === 307) {
      const location = String(res.headers()["location"] || "");
      expect(location).toMatch(/\/~/);
    }
  });
});
