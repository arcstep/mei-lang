/**
 * 静态检查 spa-navigation 模块：防止再次出现 navigate 未定义等回归。
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetsRoot = path.join(root, "app", "assets");
const modulesPath = path.join(root, "scripts", "spa-navigation-modules.json");
const moduleList = JSON.parse(await readFile(modulesPath, "utf8"));
assert.ok(Array.isArray(moduleList) && moduleList.length > 0, "spa-navigation module list required");

let src = "";
for (const rel of moduleList) {
  src += await readFile(path.join(assetsRoot, rel), "utf8");
}

assert.match(src, /function navigateInternal\(/, "navigateInternal must exist");
assert.match(src, /boot\.navigateSpa\s*=\s*function/, "boot.navigateSpa export");

const badCalls = [
  /void navigate\(/,
  /void navigate\s*\(/,
  /= navigate\(/,
  /return navigate\(/,
];
for (const re of badCalls) {
  assert.ok(!re.test(src), `forbidden bare navigate() call: ${re}`);
}

assert.match(src, /void navigateInternal\(/, "click handler must call navigateInternal");
assert.match(src, /runPostSpaWork\(/, "post-spa work must be async after DOM swap");
assert.match(src, /publishManagePreviewFromDoc\(/, "preview event after swap");

assert.match(src, /ACCESS_LIKE_ROUTE_SLUGS/, "access-like route slugs must be centralized");
assert.match(src, /BUILD_ROUTE_SLUGS/, "build route slugs must be centralized");
for (const slug of [
  "app",
  "access",
  "run",
  "access-only",
  "presentation",
  "slides",
  "build",
  "manage",
]) {
  assert.match(src, new RegExp(`"${slug.replace("-", "\\-")}"`), `route slug ${slug}`);
}
assert.match(src, /SCENE_BUNDLE_PATH_PREFIX/, "scene bundle path prefix");
assert.match(src, /findSceneBundleSrcInDoc/, "scene bundle discovery in fetched HTML");
assert.match(src, /syncSceneBundleFromDoc/, "scene bundle SPA sync");
assert.match(src, /data-mei-scene-bundle/, "scene bundle script marker");

assert.match(src, /function resolveSceneOpenRequest\(/, "resolveSceneOpenRequest must exist");
assert.match(src, /function buildSceneOpenRequest\(/, "buildSceneOpenRequest must exist");
assert.match(src, /function buildProjectionMount\(/, "buildProjectionMount must exist");
assert.match(src, /function openSceneProjection\(/, "openSceneProjection must exist");
assert.match(src, /function resolveLegacySceneProjectionConfig\(/, "legacy adapter must exist");
const overlayMatch = src.match(
  /function openProjectionOverlay\([\s\S]*?\n  function installSceneProjectionHost\(/,
);
assert.ok(overlayMatch, "openProjectionOverlay must exist");
const overlayBody = overlayMatch[0];
assert.match(overlayBody, /resolveSceneOpenRequest\(/, "openProjectionOverlay fallback via resolveSceneOpenRequest");
assert.ok(
  !/resolveDrilldownConfig|resolveLegacySceneProjectionConfig/.test(overlayBody),
  "openProjectionOverlay must not call legacy config resolver directly",
);

assert.match(src, /shouldMountDrilldownHost/, "drilldown host gate");
assert.match(
  src,
  /function resolvePreviewAppId[\s\S]*?appRouteSlugFromPathname/,
  "resolvePreviewAppId must derive app id from route slug",
);

for (const rel of moduleList) {
  const chunk = await readFile(path.join(assetsRoot, rel), "utf8");
  const lines = chunk.split("\n").length;
  assert.ok(lines <= 501, `${rel} must stay within ~500 lines (got ${lines})`);
}

console.log("spa-navigation static checks ok");
