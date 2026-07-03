import { test, expect } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.dirname(fileURLToPath(import.meta.url));
const smokeMdx = fs.readFileSync(
  path.join(rootDir, "../scripts/test/ephemeral-presentation-smoke.mdx"),
  "utf8",
);

const ACCESS_URL =
  "/apps/access/examples/slides/01-web-slides-baseline/scene/intro";

const minimalMdx = `---
presentation: inline-smoke
---

## Inline {#inline}
@composition(slides_only)
@caption
Inline compile smoke.
@end
@layout(stack)
@slot(body)
# Inline
@end
`;

test.describe("copilot ephemeral presentation", () => {
  test("compile API 返回 manifest 或结构化 diagnostics", async ({ request }) => {
    const response = await request.post("/api/presentation/compile", {
      data: {
        appId: "examples/slides/01-web-slides-baseline",
        sceneId: "intro",
        source: minimalMdx,
        mode: "ephemeral",
      },
    });
    expect([200, 422]).toContain(response.status());
    const payload = await response.json();
    expect(payload).toHaveProperty("diagnostics");
    if (response.ok()) {
      expect(payload.manifest?.steps?.length).toBeGreaterThan(0);
    } else {
      expect(Array.isArray(payload.diagnostics)).toBeTruthy();
      expect(payload.diagnostics.length).toBeGreaterThan(0);
    }
  });

  test("MeiCopilot.compileAndRunPresentation 可挂载步进与字幕", async ({ page }) => {
    const res = await page.goto(ACCESS_URL, {
      waitUntil: "domcontentloaded",
      timeout: 90000,
    });
    expect(res?.ok()).toBeTruthy();

    await page.waitForFunction(
      () =>
        window.MeiCopilot &&
        typeof window.MeiCopilot.compileAndRunPresentation === "function",
      null,
      { timeout: 30000 },
    );

    const result = await page.evaluate(async (source) => {
      try {
        const payload = await window.MeiCopilot.compileAndRunPresentation(source, {
          appId: "examples/slides/01-web-slides-baseline",
          sceneId: "intro",
          apply: true,
        });
        return {
          ok: true,
          stepCount: payload?.manifest?.steps?.length || 0,
          active: window.MeiCopilot.isActive(),
          captionVisible: !document
            .getElementById("mei-copilot-caption")
            ?.hasAttribute("hidden"),
        };
      } catch (error) {
        return {
          ok: false,
          message: String(error?.message || error),
        };
      }
    }, minimalMdx);

    expect(result.ok, result.message || "compile failed").toBeTruthy();
    expect(result.stepCount).toBeGreaterThan(0);
    expect(result.active).toBeTruthy();
    expect(result.captionVisible).toBeTruthy();
  });

  test("replace 与 clear 可清理 ephemeral session", async ({ page }) => {
    await page.goto(ACCESS_URL, {
      waitUntil: "domcontentloaded",
      timeout: 90000,
    });
    await page.waitForFunction(
      () =>
        window.MeiCopilot &&
        typeof window.MeiCopilot.replacePresentation === "function",
      null,
      { timeout: 30000 },
    );

    const replaced = await page.evaluate(async (source) => {
      const payload = await window.MeiCopilot.replacePresentation(source, {
        appId: "examples/slides/01-web-slides-baseline",
        sceneId: "intro",
        apply: true,
      });
      return payload?.manifest?.steps?.length || 0;
    }, smokeMdx);
    expect(replaced).toBeGreaterThan(1);

    const cleared = await page.evaluate(() => {
      const ok = window.MeiCopilot.clearEphemeralPresentation();
      const stored = sessionStorage.getItem("mei_copilot_presentation_v1");
      return {
        ok,
        stored,
        active: window.MeiCopilot.isActive(),
      };
    });
    expect(cleared.ok).toBeTruthy();
    expect(cleared.stored).toBeNull();
    expect(cleared.active).toBeFalsy();
  });
});
