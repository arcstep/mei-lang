/**
 * Golden-case (0537): App Runtime Instance + LaunchManifest.
 *
 * Prerequisites (optional live host):
 *   MEI_E2E_BASE_URL=http://127.0.0.1:19550 npx playwright test e2e/app-runtime-instance.spec.mjs
 *
 * Dual-profile flow (manual / CI with apply rights):
 *   apply configs/mini-data-scoped-rail.json → cutover
 *   apply configs/mini-data-full.json → second InstanceSpec → cutover / rollback
 *
 * Forced runtime (no Host data-plane fallback):
 *   MEI_APP_RUNTIME_REQUIRED=1 mei-host-shell serve --workspace … --port 19550
 */
import { test, expect } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.env.MEI_TEST_BASE_URL || "").replace(
  /\/+$/,
  "",
);
const appId = String(process.env.MEI_E2E_APP_ID || "mini-data").trim();
const dualApply = String(process.env.MEI_E2E_DUAL_APPLY || "").trim() === "1";
const scopedProfile = String(
  process.env.MEI_E2E_SCOPED_PROFILE || "mini-data-scoped-rail",
).trim();
const fullProfile = String(process.env.MEI_E2E_FULL_PROFILE || "mini-data-full").trim();

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

async function waitOpsIdle(request, timeoutMs = 300_000) {
  await expect
    .poll(
      async () => {
        const ops = await json(await request.get(`${base}/api/host/ops/status`));
        return ops.job?.status || "idle";
      },
      { timeout: timeoutMs },
    )
    .not.toBe("running");
}

async function applyProfile(request, profileId) {
  const profiles = await json(await request.get(`${base}/api/host/workspace-profiles`));
  const list = Array.isArray(profiles.profiles) ? profiles.profiles : [];
  const doc = list.find((item) => item.id === profileId) || null;
  const expectedRevision = doc?.revision || null;
  const preview = await request.post(`${base}/api/host/runtime/apply-profile`, {
    data: { profileId, dryRun: true, expectedRevision },
  });
  expect([200, 202]).toContain(preview.status());
  const accepted = await request.post(`${base}/api/host/runtime/apply-profile`, {
    data: { profileId, dryRun: false, expectedRevision },
  });
  expect([200, 202]).toContain(accepted.status());
  await waitOpsIdle(request);
  return accepted.json().catch(() => ({}));
}

test.describe("app runtime instance + LaunchManifest (0537)", () => {
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
        `${base}/api/host/view-revision?app=${encodeURIComponent(appId)}&scene=home&surface=app`,
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

  test("dual profile apply yields distinct InstanceSpecs and cutover without host restart", async ({
    request,
  }) => {
    test.skip(!dualApply, "set MEI_E2E_DUAL_APPLY=1 to run dual profile apply");

    await applyProfile(request, scopedProfile);
    const scoped = await readInstances(request);
    const scopedActive = scoped.routes?.[appId]?.active;
    expect(scopedActive).toBeTruthy();
    const scopedItem = scoped.instances.find(
      (item) => (item.instanceId || item.instance_id) === scopedActive,
    );
    const scopedDigest =
      scopedItem?.specRef ||
      scopedItem?.spec_ref ||
      scopedItem?.specDigest ||
      scopedItem?.spec_digest;

    await applyProfile(request, fullProfile);
    const full = await readInstances(request);
    const fullActive = full.routes?.[appId]?.active;
    expect(fullActive).toBeTruthy();
    expect(fullActive).not.toBe(scopedActive);
    const fullItem = full.instances.find(
      (item) => (item.instanceId || item.instance_id) === fullActive,
    );
    const fullDigest =
      fullItem?.specRef ||
      fullItem?.spec_ref ||
      fullItem?.specDigest ||
      fullItem?.spec_digest;
    if (scopedDigest && fullDigest) {
      expect(fullDigest).not.toBe(scopedDigest);
    }

    // Rollback to previous (scoped) without restarting Host.
    const rollback = await request.post(
      `${base}/api/host/routes/${encodeURIComponent(appId)}/rollback`,
      { data: {} },
    );
    expect([200, 202]).toContain(rollback.status());
    const rolled = await readInstances(request);
    expect(rolled.routes?.[appId]?.active).toBe(scopedActive);
  });
});
