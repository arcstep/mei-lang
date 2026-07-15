/**
 * Golden-case: /apps/mini-data/home under MEI_DEV_EVAL_PROFILE / MEI_EVAL_SCOPE / MEI_WARMUP_SCOPE (0535).
 *
 * Start host with matching env before running, e.g.:
 *   MEI_DEV_EVAL_PROFILE=static MEI_TEST_BASE_URL=… MEI_E2E_BASE_URL=… npx playwright test …
 *   MEI_DEV_EVAL_PROFILE=scoped MEI_EVAL_SCOPE=home/t1/r-right-rail/s-warning …
 *   MEI_DEV_EVAL_PROFILE=scoped MEI_EVAL_SCOPE=home/t1/r-right-rail/s-warning \
 *     MEI_WARMUP_SCOPE=home/t1/r-right-rail/s-warning …
 */
import { test, expect } from "@playwright/test";

const base = (process.env.MEI_E2E_BASE_URL || process.env.MEI_TEST_BASE_URL || "").replace(
  /\/+$/,
  "",
);
const profile = String(process.env.MEI_DEV_EVAL_PROFILE || "full").trim().toLowerCase();
const evalScopes = String(process.env.MEI_EVAL_SCOPE || "")
  .split(",")
  .map((part) => part.trim())
  .filter(Boolean);
const warmupScopes = String(process.env.MEI_WARMUP_SCOPE || "")
  .split(",")
  .map((part) => part.trim())
  .filter(Boolean);

function normalizeProfile(value) {
  if (value === "off" || value === "none") return "static";
  return value || "full";
}

test.describe("dev scoped eval mini-data golden", () => {
  test.skip(!base, "set MEI_E2E_BASE_URL (or MEI_TEST_BASE_URL) to run");

  test("document exposes matching __mei.dev_eval", async ({ page }) => {
    await page.goto(`${base}/apps/mini-data/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120000,
    });
    const devEval = await page.evaluate(() => window.__mei?.dev_eval || null);
    test.skip(
      !devEval,
      "host document missing __mei.dev_eval — restart mei-host-shell with this branch",
    );
    expect(String(devEval.profile || "").toLowerCase()).toBe(normalizeProfile(profile));
    if (profile === "scoped") {
      const payloadEval = Array.isArray(devEval.evalScopes)
        ? devEval.evalScopes
        : Array.isArray(devEval.scopes)
          ? devEval.scopes
          : [];
      for (const scope of evalScopes) {
        expect(payloadEval).toContain(scope);
      }
      if (warmupScopes.length) {
        expect(Array.isArray(devEval.warmupScopes)).toBe(true);
        for (const scope of warmupScopes) {
          expect(devEval.warmupScopes).toContain(scope);
        }
      }
    }
  });

  test("runtimePlan golden keeps warning hot, analytics lazy, enforcement frozen", async ({
    page,
  }) => {
    const frozenRequests = [];
    page.on("request", (request) => {
      if (!/\/api\/datasets\/(?:metrics|query)\//.test(request.url())) return;
      let payload = {};
      try {
        payload = JSON.parse(request.postData() || "{}");
      } catch (_) {}
      const encoded = JSON.stringify(payload);
      if (
        encoded.includes("enforcement") ||
        encoded.includes("key_enterprises") ||
        encoded.includes("whitelist_enterprises") ||
        encoded.includes("supervision_items_count")
      ) {
        frozenRequests.push({ url: request.url(), payload });
      }
    });
    await page.goto(`${base}/apps/mini-data/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120000,
    });
    const contract = await page.evaluate(() => {
      const plan = window.__mei?.dev_eval?.runtimePlan;
      const boot = window.__meiLangBoot || {};
      return {
        hasPlan: !!plan,
        warningWarmup: boot.devEvalAllowsWarmupScope?.(
          "home/t1/r-right-rail/s-warning",
        ),
        warningEval: boot.devEvalAllowsEvalScope?.("home/t1/r-right-rail/s-warning"),
        lazyWarmup: boot.devEvalAllowsWarmupScope?.("warnings_analytics_page"),
        lazyEval: boot.devEvalAllowsEvalScope?.("warnings_analytics_page"),
        enforcementEval: boot.devEvalAllowsEvalScope?.(
          "home/t1/r-right-rail/s-enforcement",
        ),
        warningMetric: boot.devEvalRuntimeMetricMode?.(
          "warnings_count",
          "home/t1/r-right-rail/s-warning",
        ),
        modelsMetric: boot.devEvalRuntimeMetricMode?.(
          "supervision_models_count",
          "home/t1/r-right-rail/s-warning",
        ),
        modelsHomepageAllowed: boot.devEvalAllowsMetric?.(
          "supervision_models_count",
          "home/t1/r-right-rail/s-warning",
        ),
        modelsAnalyticsAllowed: boot.devEvalAllowsMetric?.(
          "supervision_models_count",
          "supervision_models_analytics_page",
        ),
        itemsMetric: boot.devEvalRuntimeMetricMode?.(
          "supervision_items_count",
          "home/t1/r-right-rail/s-warning",
        ),
      };
    });
    test.skip(!contract.hasPlan, "requires an applied deploy.runtimePlan profile");
    expect(contract.warningWarmup).toBe(true);
    expect(contract.warningEval).toBe(true);
    expect(contract.lazyWarmup).toBe(false);
    expect(contract.lazyEval).toBe(true);
    expect(contract.enforcementEval).toBe(false);
    expect(contract.warningMetric).toBe("hot");
    expect(contract.modelsMetric).toBe("lazy");
    expect(contract.modelsHomepageAllowed).toBe(false);
    expect(contract.modelsAnalyticsAllowed).toBe(true);
    expect(contract.itemsMetric).toBe("frozen");

    await page.goto(`${base}/apps/mini-data/enforcement_analytics_page`, {
      waitUntil: "domcontentloaded",
      timeout: 120000,
    });
    await page.waitForTimeout(500);
    expect(frozenRequests).toEqual([]);
  });

  test("static profile paints placeholders without eval-pack", async ({ page }) => {
    test.skip(
      !(profile === "static" || profile === "off" || profile === "none"),
      "requires MEI_DEV_EVAL_PROFILE=static on the host under test",
    );
    const evalPack = [];
    page.on("request", (request) => {
      if (request.url().includes("/api/host/scene-eval-pack")) evalPack.push(request.url());
    });
    await page.goto(`${base}/apps/mini-data/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120000,
    });
    await page.waitForFunction(
      () =>
        document
          .querySelector("#mei-compose-root, .preview-pane-scroll")
          ?.getAttribute("data-mei-compose-materialized") === "1" ||
        window.__meiRenderPipeline?.last?.marks?.some((mark) => mark.name === "user_visible_ready"),
      { timeout: 120000 },
    );
    expect(evalPack).toEqual([]);
    const placeholders = await page.locator("[data-mei-dev-eval-placeholder='1']").count();
    expect(placeholders).toBeGreaterThan(0);
  });

  test("scoped split: warning allowed, enforcement placeholder", async ({ page }) => {
    test.skip(
      profile !== "scoped" || !evalScopes.some((scope) => scope.includes("s-warning")),
      "requires MEI_DEV_EVAL_PROFILE=scoped and MEI_EVAL_SCOPE including home/t1/r-right-rail/s-warning",
    );
    await page.goto(`${base}/apps/mini-data/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120000,
    });
    const allowed = await page.evaluate(() => {
      const boot = window.__meiLangBoot || {};
      return {
        warning: boot.devEvalAllowsEvalScope?.("home/t1/r-right-rail/s-warning"),
        enforcement: boot.devEvalAllowsEvalScope?.("home/t1/r-right-rail/s-enforcement"),
        map: boot.devEvalAllowsEvalScope?.("home/t0/r-map-stage"),
        header: boot.devEvalAllowsEvalScope?.("home/t1/r-header"),
      };
    });
    expect(allowed.warning).toBe(true);
    expect(allowed.enforcement).toBe(false);
    expect(allowed.map).toBe(false);
    expect(allowed.header).toBe(false);
  });

  test("scoped frozen sections create no runtime refs or network requests", async ({ page }) => {
    test.skip(
      profile !== "scoped" || !evalScopes.some((scope) => scope.includes("s-warning")),
      "requires scoped warning-only host",
    );
    const frozenRequests = [];
    page.on("request", (request) => {
      if (!/\/api\/datasets\/(?:metrics|query)\//.test(request.url())) return;
      let payload = {};
      try {
        payload = JSON.parse(request.postData() || "{}");
      } catch (_) {}
      const scope = String(payload.preview_scope || "");
      if (!evalScopes.some((allowed) => scope === allowed || scope.startsWith(`${allowed}/`))) {
        frozenRequests.push({ url: request.url(), scope, payload });
      }
    });
    await page.goto(`${base}/apps/mini-data/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120000,
    });
    await page.waitForFunction(
      () =>
        document
          .querySelector("#mei-compose-root, .preview-pane-scroll")
          ?.getAttribute("data-mei-compose-materialized") === "1",
      { timeout: 120000 },
    );
    await page.waitForTimeout(500);

    const frozenState = await page.evaluate(() => {
      const roots = [
        ...document.querySelectorAll('[data-preview-scope*="enforcement"]'),
      ];
      const runtimeRefs = roots.flatMap((root) =>
        [...root.querySelectorAll("[data-props]")].filter((node) => {
          try {
            return JSON.stringify(JSON.parse(node.getAttribute("data-props") || "{}")).includes(
              "__mei_runtime_ref",
            );
          } catch (_) {
            return false;
          }
        }),
      );
      return {
        placeholders: roots.filter(
          (root) => root.getAttribute("data-mei-dev-eval-placeholder") === "1",
        ).length,
        runtimeRefs: runtimeRefs.length,
      };
    });
    expect(frozenState.placeholders).toBeGreaterThan(0);
    expect(frozenState.runtimeRefs).toBe(0);
    expect(frozenRequests).toEqual([]);
  });

  test("scoped warmup set is independent from eval set", async ({ page }) => {
    test.skip(
      profile !== "scoped" || !warmupScopes.length,
      "requires MEI_DEV_EVAL_PROFILE=scoped and MEI_WARMUP_SCOPE set",
    );
    await page.goto(`${base}/apps/mini-data/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120000,
    });
    const checks = await page.evaluate(() => {
      const boot = window.__meiLangBoot || {};
      return {
        warningWarmup: boot.devEvalAllowsWarmupScope?.("home/t1/r-right-rail/s-warning"),
        warningEval: boot.devEvalAllowsEvalScope?.("home/t1/r-right-rail/s-warning"),
        enforcementWarmup: boot.devEvalAllowsWarmupScope?.("home/t1/r-right-rail/s-enforcement"),
        enforcementEval: boot.devEvalAllowsEvalScope?.("home/t1/r-right-rail/s-enforcement"),
      };
    });
    // warning: in warmupScopes (and evalScopes)
    expect(checks.warningWarmup).toBe(true);
    expect(checks.warningEval).toBe(true);
    // enforcement: in neither
    expect(checks.enforcementWarmup).toBe(false);
    expect(checks.enforcementEval).toBe(false);
  });

  test("full profile keeps default allow-all", async ({ page }) => {
    test.skip(profile !== "full" && profile !== "", "requires default/full host profile");
    await page.goto(`${base}/apps/mini-data/home`, {
      waitUntil: "domcontentloaded",
      timeout: 120000,
    });
    const allowed = await page.evaluate(() => {
      const boot = window.__meiLangBoot || {};
      return {
        warning: boot.devEvalAllowsEvalScope?.("home/t1/r-right-rail/s-warning"),
        enforcement: boot.devEvalAllowsEvalScope?.("home/t1/r-right-rail/s-enforcement"),
        warmupWarning: boot.devEvalAllowsWarmupScope?.("home/t1/r-right-rail/s-warning"),
      };
    });
    expect(allowed.warning).toBe(true);
    expect(allowed.enforcement).toBe(true);
    expect(allowed.warmupWarning).toBe(true);
  });
});
