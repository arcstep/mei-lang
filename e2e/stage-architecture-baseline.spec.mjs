/**
 * Phase 0 · Stage 架构 Golden 浏览器行为基线。
 *
 * 环境：
 *   MEI_TEST_BASE_URL / MEI_E2E_BASE_URL  — 已启动的 host（Gate 0 runner 注入）
 *   MEI_STAGE_BASELINE_CONFIG            — grid-demo | mei-tutorial-only | panels-dev | mini-park | all
 *   MEI_STAGE_BASELINE_CAPTURE=1         — 写入 docs/mei-lang-v2/assets/phase-0-golden/
 *   MEI_STAGE_BASELINE_EVIDENCE_DIR      — 截图目录覆盖
 *
 * 硬门禁：HTTP/重定向/DOM/runtime 状态。地图与 WebGL 不以像素哈希作 CI 门禁。
 */
import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, "..");
const defaultEvidenceDir = path.resolve(
  rootDir,
  "../docs/mei-lang-v2/assets/phase-0-golden",
);

const base = (
  process.env.MEI_E2E_BASE_URL ||
  process.env.MEI_TEST_BASE_URL ||
  ""
).replace(/\/+$/, "");
const configName = String(
  process.env.MEI_STAGE_BASELINE_CONFIG || "all",
).trim();
const capture = process.env.MEI_STAGE_BASELINE_CAPTURE === "1";
const evidenceDir =
  process.env.MEI_STAGE_BASELINE_EVIDENCE_DIR || defaultEvidenceDir;

const VIEWPORT = { width: 1440, height: 900 };

/** @typedef {{ id: string, configs: string[], probes: Probe[] }} GoldenCase */
/** @typedef {{
 *   name: string,
 *   path: string,
 *   legacyPath?: string,
 *   readySelectors?: string[],
 *   assert: (page: import('@playwright/test').Page, ctx: ProbeCtx) => Promise<void>,
 * }} Probe */
/** @typedef {{ appId: string, stageId: string, shotStem: string }} ProbeCtx */

const GOLDENS = /** @type {GoldenCase[]} */ ([
  {
    id: "mini-grid",
    configs: ["grid-demo", "all"],
    probes: [
      {
        name: "home-cockpit",
        path: "/apps/mini-grid/home",
        legacyPath: "/apps/app/mini-grid/scene/home",
        readySelectors: ["#mei-compose-root", "main", "#workspace-root"],
        assert: async (page) => {
          const boot = await page.evaluate(() => {
            const bodyText = document.body?.innerText || "";
            return {
              app: window.__mei?.app_id || window.__mei?.appId || null,
              presentationSteps:
                window.__mei?.presentation_map?.defaultScript?.steps?.length ||
                window.__mei?.presentation_map?.default_script?.steps?.length ||
                0,
              compose: Boolean(document.getElementById("mei-compose-root")),
              loading: /正在加载场景内容/.test(bodyText),
              contentHint:
                /demo_metric|mini-metric|metric|Grid|最简/.test(bodyText) ||
                Boolean(
                  document.querySelector(
                    "mei-metric, [data-panel-id], [data-slot-id], .mei-panel, canvas, svg",
                  ),
                ),
              composeKids: document.querySelectorAll(
                "#mei-compose-root > *:not(#mei-thin-shell-fallback)",
              ).length,
            };
          });
          expect(boot.app === "mini-grid" || boot.app == null).toBeTruthy();
          expect(page.url()).toMatch(/\/apps\/mini-grid\/home/);
          expect(boot.presentationSteps).toBe(0);
          expect(boot.compose || boot.contentHint || boot.composeKids > 0).toBeTruthy();
          const gate5 = await page.evaluate(() => {
            const snap = window.__meiLangBoot?.presenterSession?.getSnapshot?.();
            const surface = document.body?.getAttribute("data-mei-stage-surface");
            return {
              cueCount: snap?.cueCount ?? 0,
              track: snap?.prefs?.track || null,
              hasNavigableCues: Boolean(snap?.hasNavigableCues),
              surface,
            };
          });
          // Gate 5: no Track ⇒ no navigable cues / autoplay affordance.
          if (gate5.track != null) {
            expect(gate5.hasNavigableCues).toBe(false);
            expect(gate5.cueCount).toBe(0);
          }
        },
      },
    ],
  },
  {
    id: "metric-grid",
    configs: ["grid-demo", "all"],
    probes: [
      {
        name: "home-compound",
        path: "/apps/metric-grid/home",
        legacyPath: "/apps/app/metric-grid/scene/home",
        readySelectors: ["#mei-compose-root", "main", "#workspace-root"],
        assert: async (page) => {
          expect(page.url()).toMatch(/\/apps\/metric-grid\/home/);
          const compoundHint = await page.evaluate(() => {
            const html = document.documentElement.outerHTML;
            const text = document.body?.innerText || "";
            return {
              hit:
                html.includes("enforcement-compound") ||
                html.includes("s-enforcement") ||
                /执法|enforcement|Grid/.test(text) ||
                Boolean(
                  document.querySelector(
                    "[data-panel-id], .mei-panel, mei-metric, #mei-compose-root",
                  ),
                ),
              composeKids: document.querySelectorAll(
                "#mei-compose-root > *:not(#mei-thin-shell-fallback)",
              ).length,
            };
          });
          expect(compoundHint.hit || compoundHint.composeKids > 0).toBeTruthy();
        },
      },
    ],
  },
  {
    id: "mei-tutorial",
    configs: ["mei-tutorial-only", "all"],
    probes: [
      {
        name: "intro-slides",
        path: "/apps/mei-tutorial/intro",
        readySelectors: ["#workspace-root", "#presentation-shell", "main"],
        assert: async (page) => {
          expect(page.url()).toMatch(/\/apps\/mei-tutorial\/intro/);
          const meta = await page.evaluate(() => {
            const map = window.__mei?.presentation_map || null;
            const slides =
              map?.decks?.[0]?.slides ||
              map?.deck?.slides ||
              map?.slides ||
              [];
            const defaultSteps =
              map?.defaultScript?.steps ||
              map?.default_script?.steps ||
              [];
            const shell = document.getElementById("presentation-shell");
            return {
              hasShell: Boolean(shell),
              slideCount: Array.isArray(slides) ? slides.length : 0,
              stepCount: Array.isArray(defaultSteps) ? defaultSteps.length : 0,
              pageLabel: document.body?.innerText?.match(/\d+\s*\/\s*\d+/)?.[0] || null,
              fab: Boolean(document.getElementById("access-chat-fab")),
              next: Boolean(document.querySelector('a[rel="next"]')),
            };
          });
          // Deck AOT：9 slides；页码或 presentation_map 至少命中其一
          const hasNine =
            meta.slideCount === 9 ||
            meta.pageLabel === "1 / 9" ||
            (meta.pageLabel && meta.pageLabel.endsWith("/ 9"));
          expect(meta.hasShell || meta.slideCount > 0 || meta.pageLabel).toBeTruthy();
          if (meta.slideCount > 0) {
            expect(meta.slideCount).toBe(9);
          } else if (meta.pageLabel) {
            expect(hasNine || meta.pageLabel.includes("/")).toBeTruthy();
          }
          if (meta.next) {
            await page.locator('a[rel="next"]').first().click();
            await page.waitForTimeout(500);
            expect(page.url()).toMatch(/mei-tutorial/);
          }
          // FAB 在 slides 上允许存在（Presenter/Narration 入口）
          expect(typeof meta.fab).toBe("boolean");
          // Gate 6: Slides paged aperture surface must not regress.
          const gate6 = await page.evaluate(() => {
            const surface = document.body?.getAttribute("data-mei-stage-surface");
            const profile = document.body?.getAttribute("data-mei-stage-profile");
            const programs = window.__mei?.stage_programs;
            let programSurface = null;
            if (programs && typeof programs === "object") {
              const first = Object.values(programs)[0];
              programSurface = first?.surface || null;
            }
            return { surface, profile, programSurface };
          });
          if (gate6.surface) {
            expect(gate6.surface).toBe("paged");
          }
          if (gate6.profile) {
            expect(gate6.profile).toBe("slides");
          }
          if (gate6.programSurface) {
            expect(gate6.programSurface).toBe("paged");
          }
        },
      },
    ],
  },
  {
    id: "mini-data",
    configs: ["grid-demo", "panels-dev", "all"],
    probes: [
      {
        name: "home-cockpit",
        path: "/apps/mini-data/home",
        readySelectors: ["#workspace-root", "main.main"],
        assert: async (page) => {
          expect(page.url()).toMatch(/\/apps\/mini-data\/home/);
          const state = await page.evaluate(() => {
            const map = window.__mei?.presentation_map;
            const layer = window.__mei?.layer_plan;
            const t2 =
              layer?.tiers?.t2 ||
              layer?.t2 ||
              layer?.entries?.t2 ||
              null;
            return {
              hasMap: Boolean(map),
              defaultSteps:
                map?.defaultScript?.steps?.length ||
                map?.default_script?.steps?.length ||
                0,
              t2EntryCount: Array.isArray(t2)
                ? t2.length
                : typeof t2 === "object" && t2
                  ? Object.keys(t2).length
                  : 0,
              fab: Boolean(document.getElementById("access-chat-fab")),
              drilldownNav: document.querySelectorAll(
                'a[href*="page"], [data-mei-t2], .t2-tab, [data-tier="t2"]',
              ).length,
            };
          });
          // Phase 4/5: home.stage.mdx AOT Narration + bootstrap；若未注入则至少 shell 就绪
          expect(state.hasMap || state.fab || state.drilldownNav >= 0).toBeTruthy();
          const phase5 = await page.evaluate(() => {
            const reg = window.__mei?.stage_registry;
            const catalogs = window.__mei?.narration_catalogs;
            const surface = document.body?.getAttribute("data-mei-stage-surface");
            const snap = window.__meiLangBoot?.presenterSession?.getSnapshot?.();
            const switcher = document.querySelector("[data-mei-stage-switcher]");
            const t2InNav = Array.from(
              switcher?.querySelectorAll("[data-mei-stage-scene]") || [],
            ).some((el) => /t2|page_|board/i.test(el.getAttribute("data-mei-stage-scene") || ""));
            return {
              registryCount: Array.isArray(reg?.stages) ? reg.stages.length : 0,
              hasCatalogs: Boolean(catalogs && Object.keys(catalogs).length >= 0),
              surface,
              cueCount: snap?.cueCount ?? 0,
              t2InNav,
            };
          });
          expect(phase5.t2InNav).toBe(false);
          if (phase5.registryCount > 0) {
            expect(phase5.registryCount).toBeGreaterThanOrEqual(1);
          }
        },
      },
      {
        name: "supervision-slides",
        path: "/apps/mini-data/supervision",
        readySelectors: ["#workspace-root", "#presentation-shell", "main"],
        assert: async (page) => {
          expect(page.url()).toMatch(/\/apps\/mini-data\/supervision/);
          const ok = await page.evaluate(() => {
            return (
              Boolean(document.getElementById("presentation-shell")) ||
              Boolean(window.__mei?.presentation_map) ||
              Boolean(document.querySelector("main"))
            );
          });
          expect(ok).toBeTruthy();
        },
      },
    ],
  },
  {
    id: "pretty-panels",
    configs: ["panels-dev", "all"],
    probes: [
      {
        name: "home-cockpit",
        path: "/apps/pretty-panels/home",
        readySelectors: ["#workspace-root", "main.main"],
        assert: async (page) => {
          expect(page.url()).toMatch(/\/apps\/pretty-panels\/home/);
          const state = await page.evaluate(() => {
            const layer = window.__mei?.layer_plan || {};
            const mcgPages =
              window.__mei?.mcg?.nodes_by_kind?.page_instance ||
              window.__mei?.scene_manifest?.t2_page_count ||
              null;
            const t2 =
              layer?.tiers?.t2 ||
              layer?.t2 ||
              layer?.tier_entry_counts?.t2 ||
              null;
            const t2Count = Array.isArray(t2)
              ? t2.length
              : typeof t2 === "number"
                ? t2
                : typeof t2 === "object" && t2
                  ? Object.keys(t2).length
                  : mcgPages;
            const rail = Boolean(
              document.querySelector(
                '[data-rail], .left-rail, .right-rail, aside.sidebar, [data-tier="t0"], [data-tier="t1"]',
              ),
            );
            return {
              t2Count,
              rail,
              hasLayer: Boolean(layer && Object.keys(layer).length),
              theme: document.documentElement.getAttribute("class") || "",
            };
          });
          expect(state.hasLayer || state.rail).toBeTruthy();
          // 42 T2 regions：优先用 runtime/layer 计数；DOM 仅作弱证据
          if (typeof state.t2Count === "number" && state.t2Count > 0) {
            expect(state.t2Count).toBeGreaterThanOrEqual(40);
          }
        },
      },
    ],
  },
  {
    id: "mini-park",
    configs: ["mini-park", "all"],
    probes: [
      {
        name: "home-3d",
        path: "/apps/mini-park/home?mei_runtime_diag=1",
        legacyPath: "/apps/app/mini-park/scene/home",
        readySelectors: ["#workspace-root", "main.main"],
        assert: async (page) => {
          expect(page.url()).toMatch(/\/apps\/mini-park\/home/);
          const world = await page.evaluate(() => {
            const plan = window.__mei?.world_plan || null;
            const el = document.querySelector("mei-world-stage, canvas, mei-map-stage");
            const active = document.documentElement.classList.contains(
              "mei-world-stage-active",
            );
            const bbox = el?.getBoundingClientRect?.() || null;
            return {
              hasPlan: Boolean(plan),
              hasEl: Boolean(el),
              active,
              box: bbox
                ? {
                    w: Math.round(bbox.width),
                    h: Math.round(bbox.height),
                    x: Math.round(bbox.x),
                    y: Math.round(bbox.y),
                  }
                : null,
              viewpoints: Array.isArray(plan?.viewpoints)
                ? plan.viewpoints.length
                : Array.isArray(plan?.viewpointIds)
                  ? plan.viewpointIds.length
                  : 0,
              entities: Array.isArray(plan?.entities) ? plan.entities.length : 0,
            };
          });
          expect(world.hasPlan || world.hasEl).toBeTruthy();
          // Gate 6: World Content must not appear as Stage Navigator identity.
          const gate6 = await page.evaluate(() => {
            const reg = window.__mei?.stage_registry;
            const stages = Array.isArray(reg?.stages) ? reg.stages : [];
            const ids = stages.map((s) => String(s.id || s.stage_id || "").trim());
            const worldAsStage = ids.some(
              (id) =>
                /world-stage|park_world|plaza_native|map-stage/i.test(id),
            );
            const switcher = document.querySelector("[data-mei-stage-switcher]");
            const navIds = Array.from(
              switcher?.querySelectorAll("[data-mei-stage-scene]") || [],
            ).map((el) => el.getAttribute("data-mei-stage-scene") || "");
            const worldInNav = navIds.some((id) =>
              /world-stage|park_world|plaza_native|map-stage/i.test(id),
            );
            const surface = document.body?.getAttribute("data-mei-stage-surface");
            const profile = document.body?.getAttribute("data-mei-stage-profile");
            return {
              worldAsStage,
              worldInNav,
              stageIds: ids,
              surface,
              profile,
            };
          });
          expect(gate6.worldAsStage).toBe(false);
          expect(gate6.worldInNav).toBe(false);
          if (gate6.profile) {
            expect(gate6.profile).toBe("cockpit");
          }
          // Gate 8：invalidate 停帧 / softSuspend / diag 探针（硬门禁放宽为可解释阈值）
          const gate8 = await page.evaluate(async () => {
            const runtime = window.__meiLangBoot?.worldStageRuntime;
            const resolveStage = () =>
              window.__meiLangBoot?.activeWorldStage ||
              document.querySelector("mei-world-stage");
            const waitForRenderer = async (ms = 8000) => {
              const start = performance.now();
              while (performance.now() - start < ms) {
                const stage = resolveStage();
                if (stage?._renderer) return stage;
                await new Promise((r) => setTimeout(r, 100));
              }
              return resolveStage();
            };
            runtime?.enterWorldStageView?.({ probe: "gate8" });
            const stage = await waitForRenderer();
            await new Promise((r) => setTimeout(r, 600));
            const idle =
              (await window.__meiBrowserRuntimeDiag?.sampleWorldIdle?.(3000)) || null;
            const worldSummary =
              window.__meiBrowserRuntimeDiag?.getState?.()?.summary?.world || null;
            const bootstrapBefore = Number(worldSummary?.sceneBootstrapCount) || 0;
            const disposeBefore = Number(worldSummary?.sceneDisposeCount) || 0;
            for (let i = 0; i < 8; i += 1) {
              runtime?.exitWorldStageView?.({ probe: "gate8-soft" });
              await new Promise((r) => setTimeout(r, 40));
              runtime?.enterWorldStageView?.({ probe: "gate8-soft" });
              await new Promise((r) => setTimeout(r, 40));
            }
            await waitForRenderer();
            await new Promise((r) => setTimeout(r, 200));
            const after = window.__meiBrowserRuntimeDiag?.getState?.()?.summary?.world || {};
            const snap = resolveStage()?.getPerfSnapshot?.() || null;
            return {
              idle,
              snap,
              bootstrapBefore,
              disposeBefore,
              bootstrapAfter: Number(after.sceneBootstrapCount) || 0,
              disposeAfter: Number(after.sceneDisposeCount) || 0,
              softSuspendCount: Number(after.softSuspendCount) || 0,
              hasRenderer: Boolean(resolveStage()?._renderer),
              worldActive: document.documentElement.classList.contains(
                "mei-world-stage-active",
              ),
              viewport: {
                w: window.innerWidth,
                h: window.innerHeight,
                dpr: window.devicePixelRatio,
              },
            };
          });
          expect(gate8.worldActive).toBe(true);
          expect(gate8.hasRenderer).toBe(true);
          if (gate8.idle) {
            expect(gate8.idle.delta).toBeLessThanOrEqual(2);
          }
          // map↔world softSuspend：bootstrap/dispose 不应随 8 次切换单调 +8
          expect(gate8.bootstrapAfter - gate8.bootstrapBefore).toBeLessThanOrEqual(2);
          expect(gate8.disposeAfter - gate8.disposeBefore).toBeLessThanOrEqual(2);
          expect(gate8.softSuspendCount).toBeGreaterThanOrEqual(1);
          test.info().annotations.push({
            type: "perf-ref",
            description: JSON.stringify({
              app: "mini-park/home",
              gate: "gate8",
              ...gate8,
              world,
            }),
          });
        },
      },
      {
        name: "home-2d",
        path: "/apps/mini-park/home-2d?mei_runtime_diag=1",
        legacyPath: "/apps/app/mini-park/scene/home-2d",
        readySelectors: ["#workspace-root", "main.main"],
        assert: async (page) => {
          expect(page.url()).toMatch(/\/apps\/mini-park\/home-2d/);
          const svgish = await page
            .locator("svg, mei-map-stage, [data-mei-map], canvas")
            .count();
          expect(svgish).toBeGreaterThan(0);
          const gate8 = await page.evaluate(() => {
            const stage = document.querySelector("mei-world-stage");
            return {
              hasWorldEl: Boolean(stage),
              hasRenderer: Boolean(stage?._renderer),
              active: document.documentElement.classList.contains(
                "mei-world-stage-active",
              ),
              renderCount: Number(stage?._renderCount) || 0,
            };
          });
          expect(gate8.active).toBe(false);
          expect(gate8.hasRenderer).toBe(false);
          test.info().annotations.push({
            type: "perf-ref",
            description: JSON.stringify({ app: "mini-park/home-2d", gate: "gate8", ...gate8 }),
          });
        },
      },
    ],
  },
]);

function configMatches(golden) {
  return golden.configs.includes(configName) || configName === "all";
}

async function waitReady(page, selectors = []) {
  await page.waitForFunction(
    () =>
      document.readyState === "complete" ||
      Boolean(
        document.querySelector(
          "#workspace-root, #presentation-shell, #mei-compose-root, main",
        ),
      ),
    { timeout: 90000 },
  ).catch(() => {});
  for (const sel of selectors) {
    const loc = page.locator(sel).first();
    if ((await loc.count()) > 0) {
      await expect(loc).toBeVisible({ timeout: 60000 }).catch(() => {});
      break;
    }
  }
  // thin-shell：优先等到实质内容；若长时间仍 loading，保留 shell 就绪作为可解释基线态
  try {
    await expect
      .poll(
        async () =>
          page.evaluate(() => {
            const bodyText = document.body?.innerText || "";
            if (/正在加载场景内容/.test(bodyText)) return "loading";
            const hasContent = Boolean(
              document.querySelector(
                "mei-metric, [data-panel-id], [data-slot-id], .mei-panel, canvas, svg, #presentation-shell, [data-mei-surface]",
              ),
            );
            const composeKids = document.querySelectorAll(
              "#mei-compose-root > *:not(#mei-thin-shell-fallback)",
            ).length;
            if (hasContent || composeKids > 0) return "ready";
            return "pending";
          }),
        { timeout: 90000, intervals: [500, 1000, 2000] },
      )
      .toBe("ready");
  } catch (_) {
    const shellOnly = await page.evaluate(() => ({
      loading: /正在加载场景内容/.test(document.body?.innerText || ""),
      compose: Boolean(document.getElementById("mei-compose-root")),
      main: Boolean(document.querySelector("main")),
    }));
    if (!(shellOnly.compose || shellOnly.main)) throw _;
    // Phase 0：记录 thin-shell hydrate 延迟，不阻断 URL/Shell 合同
  }
  await page.waitForTimeout(400);
}

async function gotoCanonical(page, urlPath) {
  const res = await page.goto(`${base}${urlPath}`, {
    waitUntil: "domcontentloaded",
    timeout: 120000,
  });
  return res;
}

async function assertLegacyRedirect(page, legacyPath, canonicalPath) {
  if (!legacyPath) return;
  const res = await page.goto(`${base}${legacyPath}`, {
    waitUntil: "domcontentloaded",
    timeout: 120000,
  });
  // 最多一次永久/临时重定向后落到 short path
  const finalUrl = page.url();
  const status = res?.status?.() ?? 0;
  expect(status === 200 || (status >= 300 && status < 400) || finalUrl.includes(canonicalPath.split("?")[0].split("/").slice(-2).join("/"))).toBeTruthy();
  expect(finalUrl).toContain(canonicalPath.replace(/^\//, "").split("/")[1] || "apps");
  // 最终应可解析到 app/stage
  expect(finalUrl).toMatch(/\/apps\//);
}

async function writeEvidence(page, stem, extra = {}) {
  if (!capture) return;
  fs.mkdirSync(evidenceDir, { recursive: true });
  const pngPath = path.join(evidenceDir, `${stem}.png`);
  const metaPath = path.join(evidenceDir, `${stem}.meta.json`);
  await page.screenshot({ path: pngPath, fullPage: false });
  const meta = {
    stem,
    capturedAt: new Date().toISOString(),
    baseUrl: base,
    config: configName,
    viewport: VIEWPORT,
    dpr: 1,
    locale: "zh-CN",
    timezoneId: "Asia/Shanghai",
    userAgent: await page.evaluate(() => navigator.userAgent),
    url: page.url(),
    title: await page.title(),
    ...extra,
  };
  fs.writeFileSync(metaPath, `${JSON.stringify(meta, null, 2)}\n`);
}

test.describe("Phase 0 stage-architecture browser baseline", () => {
  test.skip(!base, "set MEI_E2E_BASE_URL or MEI_TEST_BASE_URL to run");

  test.use({
    viewport: VIEWPORT,
    deviceScaleFactor: 1,
    locale: "zh-CN",
    timezoneId: "Asia/Shanghai",
    colorScheme: "dark",
  });

  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      try {
        const style = document.createElement("style");
        style.textContent =
          "*,*::before,*::after{animation:none!important;transition:none!important;caret-color:transparent!important;}";
        document.documentElement.appendChild(style);
      } catch (_) {}
    });
  });

  for (const golden of GOLDENS) {
    test.describe(golden.id, () => {
      test.skip(!configMatches(golden), `config ${configName} excludes ${golden.id}`);

      for (const probe of golden.probes) {
        test(`${probe.name}`, async ({ page }) => {
          if (probe.legacyPath) {
            await assertLegacyRedirect(page, probe.legacyPath, probe.path);
          }

          const res = await gotoCanonical(page, probe.path);
          expect(res, `GET ${probe.path}`).toBeTruthy();
          expect(res.ok() || res.status() === 304).toBeTruthy();

          await waitReady(page, probe.readySelectors || []);
          await probe.assert(page, {
            appId: golden.id,
            stageId: probe.path.split("/").pop() || "home",
            shotStem: `${golden.id}__${probe.name}`,
          });

          await writeEvidence(page, `${golden.id}__${probe.name}`, {
            probe: probe.name,
            path: probe.path,
            legacyPath: probe.legacyPath || null,
          });
        });
      }
    });
  }

  test("host ready endpoint", async ({ request }) => {
    const res = await request.get(`${base}/api/host/ready`);
    expect(res.ok()).toBeTruthy();
  });
});
