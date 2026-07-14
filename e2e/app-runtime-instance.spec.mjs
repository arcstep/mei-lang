/**
 * Golden-case (0537): App Launch Config + single Runtime per app.
 *
 * Prerequisites:
 *   MEI_E2E_BASE_URL=http://127.0.0.1:19550 npx playwright test e2e/app-runtime-instance.spec.mjs
 *
 * Host start example:
 *   mei-host-shell serve --workspace ../workspaces/ws-demo-v2 --port 19550 \
 *     --app-config apps/mini-data/launch/scoped-rail.json
 *
 * Switch config (stop+start): MEI_E2E_DUAL_APPLY=1 uses apps start API with scoped-rail then full.
 */
import { test, expect } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.env.MEI_TEST_BASE_URL || "").replace(
  /\/+$/,
  "",
);
const appId = String(process.env.MEI_E2E_APP_ID || "mini-data").trim();
const dualApply = String(process.env.MEI_E2E_DUAL_APPLY || "").trim() === "1";

async function json(response) {
  expect(response.ok(), `${response.status()} ${response.url()}`).toBeTruthy();
  return response.json();
}

async function readManifest(request) {
  return json(await request.get(`${base}/api/host/launch-manifest`));
}

async function readInstances(request) {
  return json(await request.get(`${base}/api/host/instances`));
}

test.describe("app launch config + single runtime (0537)", () => {
  test.skip(!base, "set MEI_E2E_BASE_URL (or MEI_TEST_BASE_URL) to run");

  test("launch-manifest and instances expose routes and observed state", async ({
    request,
  }) => {
    const manifest = await readManifest(request);
    expect(typeof manifest.revision).toBe("string");
    expect(manifest.manifest).toBeTruthy();
    expect(manifest.manifest.routes || {}).toBeTruthy();

    const instances = await readInstances(request);
    expect(instances.revision).toBe(manifest.revision);
    expect(Array.isArray(instances.instances)).toBe(true);
    expect(instances.routes || {}).toBeTruthy();

    const route = instances.routes?.[appId] || manifest.manifest.routes?.[appId];
    test.skip(!route?.active, `no active LaunchManifest route for ${appId}`);

    const active = instances.instances.find(
      (item) => item.instanceId === route.active || item.instance_id === route.active,
    );
    expect(active).toBeTruthy();
    expect(active.appId || active.app_id || appId).toBeTruthy();
    expect(["active", "candidate", "previous", null, undefined]).toContain(
      active.routeRole || active.route_role || "active",
    );
  });

  test("cutover rejects stale manifest revision (CAS)", async ({ request }) => {
    const instances = await readInstances(request);
    const route = instances.routes?.[appId];
    const candidate = route?.candidate;
    test.skip(!candidate, `no candidate route for ${appId} — skip CAS conflict probe`);

    const conflict = await request.post(
      `${base}/api/host/routes/${encodeURIComponent(appId)}/cutover`,
      {
        data: {
          instanceId: candidate,
          expectedManifestRevision: "stale-revision-for-e2e",
        },
      },
    );
    expect(conflict.status()).toBe(409);
  });

  test("rollback restores previous when warm standby exists", async ({ request }) => {
    const before = await readInstances(request);
    const route = before.routes?.[appId];
    test.skip(!route?.previous, `no previous standby for ${appId}`);

    const previousId = route.previous;
    const activeBefore = route.active;
    const rollback = await request.post(
      `${base}/api/host/routes/${encodeURIComponent(appId)}/rollback`,
      { data: {} },
    );
    expect([200, 202]).toContain(rollback.status());

    const after = await readInstances(request);
    expect(after.routes?.[appId]?.active).toBe(previousId);
    expect(after.routes?.[appId]?.previous).toBe(activeBefore);

    // Restore original active so other tests keep a stable baseline.
    const restore = await request.post(
      `${base}/api/host/routes/${encodeURIComponent(appId)}/rollback`,
      { data: {} },
    );
    expect([200, 202]).toContain(restore.status());
  });

  test("Access home stays coherent with active generation after route settle", async ({
    page,
    request,
  }) => {
    const instances = await readInstances(request);
    const route = instances.routes?.[appId];
    test.skip(!route?.active, `no active route for ${appId}`);

    const active = instances.instances.find(
      (item) => (item.instanceId || item.instance_id) === route.active,
    );
    const generation =
      active?.resource?.generation ||
      active?.revisions?.dataGeneration ||
      active?.revisions?.data_generation ||
      null;

    const revision = await json(
      await request.get(
        `${base}/api/host/view-revision?app_id=${encodeURIComponent(appId)}&scene=home&surface=app`,
      ),
    );

    await page.goto(`${base}/apps/${encodeURIComponent(appId)}/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120_000,
    });
    const client = await page.evaluate(() => ({
      envelope: window.__mei?.view_revision_envelope || null,
      footer: document.querySelector("#mei-status-host-version")?.textContent?.trim() || "",
    }));
    if (client.envelope?.registry_revision) {
      expect(client.envelope.registry_revision).toBe(revision.registry_revision);
    }
    if (generation && client.footer) {
      expect(client.footer).toContain(generation);
    }
  });

  test("running overview exposes launch display fields", async ({ request }) => {
    const apps = await json(await request.get(`${base}/api/host/apps`));
    expect(Array.isArray(apps.apps)).toBe(true);
    expect(Array.isArray(apps.running)).toBe(true);
    const run = (apps.running || []).find((row) => row.appId === appId);
    test.skip(!run, `no running row for ${appId}`);
    expect(run.href).toBe(`/apps/${appId}/home`);
    expect(typeof run.displayName).toBe("string");
    expect(run.displayName.length).toBeGreaterThan(0);
    expect(run.launchId || run.instanceId).toBeTruthy();
  });

  test("shell-chrome topbar lists only running apps", async ({ request }) => {
    const chrome = await json(
      await request.get(
        `${base}/api/host/shell-chrome?appId=${encodeURIComponent(appId)}&scene=home&surface=app`,
      ),
    );
    expect(typeof chrome.topbarHtml).toBe("string");
    expect(typeof chrome.digest).toBe("string");
    const runningIds = chrome.runningAppIds || [];
    expect(Array.isArray(runningIds)).toBe(true);
    if (runningIds.length === 0) {
      // --launch none: no app tabs in topbar slot content.
      expect(chrome.topbarHtml.includes(`data-mei-app-id="${appId}"`) || chrome.topbarHtml.includes(`/apps/${appId}/`)).toBeFalsy();
      return;
    }
    expect(runningIds).toContain(appId);
    expect(chrome.topbarHtml).toContain(`/apps/${appId}/home`);
  });

  test("Access topbar shows launch displayName for running app", async ({ page, request }) => {
    const apps = await json(await request.get(`${base}/api/host/apps`));
    const run = (apps.running || []).find((row) => row.appId === appId);
    test.skip(!run, `no running row for ${appId}`);

    await page.goto(`${base}/apps/${encodeURIComponent(appId)}/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120_000,
    });
    const top = page.locator("#mei-host-topbar-slot");
    await expect(top).toBeVisible({ timeout: 60_000 });
    await expect(top).toContainText(run.displayName, { timeout: 60_000 });
  });

  test("switch launch config stops old process and starts new one", async ({ request }) => {
    test.skip(!dualApply, "set MEI_E2E_DUAL_APPLY=1 to run launch config switch");

    const startScoped = await request.post(
      `${base}/api/host/apps/${encodeURIComponent(appId)}/start`,
      { data: { config: "scoped-rail" } },
    );
    expect([200, 202]).toContain(startScoped.status());
    const apps1 = await json(await request.get(`${base}/api/host/apps`));
    const run1 = (apps1.running || []).find((row) => row.appId === appId);
    expect(run1?.instanceId).toBeTruthy();
    expect(run1?.launchId).toBe("scoped-rail");
    expect(String(run1?.displayName || "")).toMatch(/scoped/i);

    const startFull = await request.post(
      `${base}/api/host/apps/${encodeURIComponent(appId)}/start`,
      { data: { config: "full" } },
    );
    expect([200, 202]).toContain(startFull.status());
    const apps2 = await json(await request.get(`${base}/api/host/apps`));
    const run2 = (apps2.running || []).find((row) => row.appId === appId);
    expect(run2?.instanceId).toBeTruthy();
    expect(run2.instanceId).not.toBe(run1.instanceId);
    expect(run2?.launchId).toBe("full");
    expect(String(run2?.displayName || "")).not.toBe(String(run1?.displayName || ""));

    const chrome = await json(
      await request.get(
        `${base}/api/host/shell-chrome?appId=${encodeURIComponent(appId)}&scene=home`,
      ),
    );
    expect(chrome.topbarHtml).toContain(run2.displayName);

    const stop = await request.post(
      `${base}/api/host/apps/${encodeURIComponent(appId)}/stop`,
      { data: {} },
    );
    expect([200, 202]).toContain(stop.status());
    const apps3 = await json(await request.get(`${base}/api/host/apps`));
    expect((apps3.running || []).find((row) => row.appId === appId)).toBeFalsy();
  });
});
