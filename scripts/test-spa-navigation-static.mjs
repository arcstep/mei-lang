/**
 * 静态检查 spa-navigation 模块：防止再次出现 navigate 未定义等回归。
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetsRoot = path.join(root, "app", "assets");

// View Assembly Runtime bundle checks (independent of spa-navigation module concat)
const bundleManifestPath = path.join(root, "scripts", "bundle-manifest.json");
const bundleManifest = JSON.parse(await readFile(bundleManifestPath, "utf8"));
const accessScripts = bundleManifest.accessScripts || [];
const manageScripts = bundleManifest.manageScripts || [];
const assemblyModules = [
  "spa-navigation/spa/structure-tree-materializer.js",
  "spa-navigation/spa/host-capabilities-ready.js",
  "spa-navigation/spa/surface-ready.js",
  "spa-navigation/spa/view-assembly-coordinator.js",
  "spa-navigation/spa/ensure-surface-runtime.js",
];
for (const mod of assemblyModules) {
  assert.ok(accessScripts.includes(mod), `access bundle must include ${mod}`);
  assert.ok(manageScripts.includes(mod), `manage bundle must include ${mod}`);
}
const sceneCacheDiag = "spa-navigation/spa/scene-cache-diag.js";
assert.ok(accessScripts.includes(sceneCacheDiag), "scene-cache-diag must stay in access bundle");
assert.ok(
  !manageScripts.includes(sceneCacheDiag),
  "scene-cache-diag must not be duplicated in manage bundle",
);
const coordinatorSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/view-assembly-coordinator.js"),
  "utf8",
);
assert.match(coordinatorSrc, /boot\.viewAssembly\s*=\s*\{[\s\S]*assemble/, "coordinator must export assemble");
const previewIdx = coordinatorSrc.indexOf("tryCacheFirstViewRestore");
const verifyIdx = coordinatorSrc.indexOf("await phaseVerify(");
const chromeIdx = coordinatorSrc.indexOf("await phaseChrome(");
assert.ok(previewIdx >= 0 && verifyIdx > previewIdx && chromeIdx > verifyIdx, "phaseVerify must run after preview and before chrome");
assert.match(coordinatorSrc, /surfaceSwitch/, "unified cold_start surfaceSwitch flag required");
assert.match(coordinatorSrc, /normalizeAssemblyOpts/, "normalizeAssemblyOpts required");

const surfaceReadySrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/surface-ready.js"),
  "utf8",
);
assert.match(surfaceReadySrc, /boot\.isSurfaceMaterialized\s*=/, "isSurfaceMaterialized export required");
assert.match(surfaceReadySrc, /boot\.surfaceSnapshot\s*=/, "surfaceSnapshot export required");

const viewRevisionClientSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/view-revision-client.js"),
  "utf8",
);
assert.match(viewRevisionClientSrc, /defaultReviewProjectionForSurface/, "compose defaults required");
assert.match(viewRevisionClientSrc, /omit_digests/, "surface_switch omit_digests required");

const revisionContractSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/revision-contract.js"),
  "utf8",
);
assert.match(revisionContractSrc, /ssrManifestMatchesSurface/, "cross-surface digest guard required");
assert.match(revisionContractSrc, /mergeSemanticManifestLayers/, "semantic manifest merge required");
assert.match(revisionContractSrc, /replaceSurfaceManifestSlice/, "surface manifest slice replace required");
assert.match(revisionContractSrc, /applySceneManifestRefs/, "applySceneManifestRefs required");

const thinShellHostSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/thin-shell-host.js"),
  "utf8",
);
assert.match(thinShellHostSrc, /hostChromeReady/, "hostChromeReady export required");
assert.match(thinShellHostSrc, /isSsrShellPlaceholder/, "isSsrShellPlaceholder export required");

const viewCompositorSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/view-compositor.js"),
  "utf8",
);
assert.match(viewCompositorSrc, /isPlaceholderShellDoc/, "isPlaceholderShellDoc export required");

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

const ASSEMBLY_LINE_COUNT_TARGETS = [
  "spa-navigation/spa/structure-tree-materializer.js",
  "spa-navigation/spa/host-capabilities-ready.js",
  "spa-navigation/spa/view-assembly-coordinator.js",
];

for (const rel of ASSEMBLY_LINE_COUNT_TARGETS) {
  const chunk = await readFile(path.join(assetsRoot, rel), "utf8");
  const lines = chunk.split("\n").length;
  assert.ok(lines <= 520, `${rel} must stay within ~500 lines (got ${lines})`);
}

console.log("spa-navigation static checks ok");
console.log("view-assembly bundle checks ok");
