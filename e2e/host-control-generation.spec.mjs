import { test, expect } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.env.MEI_TEST_BASE_URL || "").replace(
  /\/+$/,
  "",
);
const generation = String(process.env.MEI_E2E_GENERATION_TARGET || "").trim();
const appId = String(process.env.MEI_E2E_APP_ID || "mini-data").trim();

async function json(response) {
  expect(response.ok(), `${response.status()} ${response.url()}`).toBeTruthy();
  return response.json();
}

test.describe("host control generation coherence", () => {
  test.skip(!base, "set MEI_E2E_BASE_URL (or MEI_TEST_BASE_URL) to run");

  test("generation activation keeps APIs, client envelope, and footer coherent", async ({
    page,
    request,
  }) => {
    test.skip(!generation, "set MEI_E2E_GENERATION_TARGET to run activation coverage");
    const accepted = await request.post(
      `${base}/api/host/builds/${encodeURIComponent(generation)}/activate`,
      { data: {} },
    );
    expect([200, 202]).toContain(accepted.status());

    await expect
      .poll(
        async () => {
          const ops = await json(await request.get(`${base}/api/host/ops/status`));
          return ops.job?.status || "idle";
        },
        { timeout: 180_000 },
      )
      .not.toBe("running");

    const [version, revision, bootstrap] = await Promise.all([
      json(await request.get(`${base}/api/host/version`)),
      json(
        await request.get(
          `${base}/api/host/view-revision?app=${encodeURIComponent(appId)}&scene=home&surface=app`,
        ),
      ),
      json(
        await request.get(
          `${base}/api/host/scene-bootstrap?app=${encodeURIComponent(appId)}&scene=home`,
        ),
      ),
    ]);
    expect(version.workspace?.env?.currentByApp?.[appId]).toBe(generation);

    await page.goto(`${base}/apps/${encodeURIComponent(appId)}/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120_000,
    });
    const client = await page.evaluate(() => ({
      envelope: window.__mei?.view_revision_envelope || null,
      footer: document.querySelector("#mei-status-host-version")?.textContent?.trim() || "",
    }));
    expect(client.envelope?.registry_revision).toBe(revision.registry_revision);
    expect(client.envelope?.client_revision).toBe(
      bootstrap.clientRevision || bootstrap.client_revision,
    );
    expect(client.footer).toContain(generation);
  });

  test("cleanup preview never marks protected generations removable", async ({ request }) => {
    const preview = await json(
      await request.post(`${base}/api/host/builds/cleanup-preview`, { data: {} }),
    );
    const entries = preview.report?.entries || [];
    for (const entry of entries.filter((item) => item.protected)) {
      expect(entry.reasons?.length || 0).toBeGreaterThan(0);
      expect(preview.report?.removed || []).not.toContain(entry.path);
    }
  });
});
