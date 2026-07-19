/**
 * 静态检查 spa-navigation 模块：防止再次出现 navigate 未定义等回归。
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const assetsRoot = path.join(root, "host-shell", "app", "assets");
const runtimeQuerySrc = await readFile(
  path.join(root, "stock/components/dataset/runtime-query.js"),
  "utf8",
);
assert.match(
  runtimeQuerySrc,
  /scopeBootstrap\?\.bootstrap_metrics/,
  "bootstrap seeding must accept top-level bootstrap_metrics",
);
assert.match(
  runtimeQuerySrc,
  /delete scopePayload\.preview_scope/,
  "metric scope cache must share warmed results across component mounts",
);
assert.match(
  runtimeQuerySrc,
  /delete normalized\.preview_scope/,
  "dataset cache must share warmed page-one results across component mounts",
);

// View Assembly Runtime bundle checks (independent of spa-navigation module concat)
const bundleManifestPath = path.join(root, "scripts", "build", "bundle-manifest.json");
const bundleManifest = JSON.parse(await readFile(bundleManifestPath, "utf8"));
const accessScripts = bundleManifest.accessScripts || [];
const manageScripts = bundleManifest.manageScripts || [];
const adminScripts = bundleManifest.adminScripts || [];
assert.ok(
  adminScripts.includes("visit-history-panel.js"),
  "admin bundle must mount the shared visit-history panel",
);
const visitHistoryPanelSrc = await readFile(
  path.join(assetsRoot, "visit-history-panel.js"),
  "utf8",
);
assert.match(
  visitHistoryPanelSrc,
  /function recordAdminVisit[\s\S]*ctx\.routeKind !== "admin"[\s\S]*api\.append/,
  "admin pages must append route-aware visit history records",
);
const objectSelectionModule =
  "spa-navigation/presentation/object-selection-runtime.js";
const mapWorldBridgeModule = "spa-navigation/presentation/map-world-bridge.js";
const focusControllerModule = "spa-navigation/presentation/focus-controller.js";
for (const scripts of [accessScripts, manageScripts]) {
  const selectionIndex = scripts.indexOf(objectSelectionModule);
  const bridgeIndex = scripts.indexOf(mapWorldBridgeModule);
  const focusIndex = scripts.indexOf(focusControllerModule);
  assert.ok(selectionIndex >= 0, `bundle must include ${objectSelectionModule}`);
  assert.ok(
    selectionIndex < bridgeIndex && selectionIndex < focusIndex,
    "object selection runtime must load before map-world and focus bridges",
  );
}
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

const drilldownContextLoaderSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/drilldown-context-loader.js"),
  "utf8",
);
assert.match(
  drilldownContextLoaderSrc,
  /function isCacheableDrilldownRevision[\s\S]*__no_client_bootstrap__/,
  "drilldown context must reject placeholder revisions",
);
assert.match(
  drilldownContextLoaderSrc,
  /const cacheable = isCacheableDrilldownRevision\(revision\);[\s\S]*if \(cacheable\) \{[\s\S]*readSessionDrilldown/,
  "drilldown context must bypass stale session cache without a content revision",
);
assert.match(
  drilldownContextLoaderSrc,
  /boot\.reportDrilldownContextError\s*=\s*reportDrilldownContextError/,
  "drilldown context failures must expose the Host reporting bridge",
);

const thinShellHostSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/thin-shell-host.js"),
  "utf8",
);
assert.match(thinShellHostSrc, /hostChromeReady/, "hostChromeReady export required");
assert.match(thinShellHostSrc, /isHostChromeSuppressed/, "chrome=none topbar suppression required");
assert.match(thinShellHostSrc, /isSsrShellPlaceholder/, "isSsrShellPlaceholder export required");
assert.match(
  thinShellHostSrc,
  /function hostChromeReady\(ctx\)[\s\S]*isHostChromeSuppressed\(ctx\)[\s\S]*summary\.statusbar/,
  "chrome=none must wait for the persistent statusbar",
);
assert.match(
  thinShellHostSrc,
  /isHostChromeSuppressed\(chromeCtx\)[\s\S]*statusbar_html[\s\S]*topSlot\.innerHTML = ""/,
  "chrome=none must suppress only the topbar and still apply the statusbar",
);

assert.match(
  surfaceReadySrc,
  /hostChromeReady\(ctx\)/,
  "isSurfaceMaterialized must pass ctx into hostChromeReady",
);

const viewCompositorSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/view-compositor.js"),
  "utf8",
);
assert.match(viewCompositorSrc, /isPlaceholderShellDoc/, "isPlaceholderShellDoc export required");

const previewMaterializerSrc = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/preview-materializer.js"),
  "utf8",
);
assert.match(
  previewMaterializerSrc,
  /props\.__mei_layout_fill[\s\S]*data-mei-layout-fill/,
  "compiled fill-down marker must be projected to DOM",
);
assert.match(
  previewMaterializerSrc,
  /\/app-bundles\/access\.js[\s\S]*\/workspace-app-assets\/[\s\S]*encodeURIComponent\(version\)/,
  "mutable workspace backgrounds must use the active Runtime asset version",
);
assert.doesNotMatch(
  previewMaterializerSrc,
  /applyEnforcementSectionComposeClasses/,
  "fill-down sizing must not depend on enforcement scope heuristics",
);
const appShellCss = await readFile(path.join(assetsRoot, "app-shell.css"), "utf8");
const topbarMenuSrc = await readFile(
  path.join(assetsRoot, "topbar-app-group-menu.js"),
  "utf8",
);
assert.doesNotMatch(
  topbarMenuSrc,
  /scrollActiveChipsIntoView/,
  "topbar must not restore the removed horizontal chip scroller",
);
assert.match(
  topbarMenuSrc,
  /topbar-more-dropdown[\s\S]*preferredWidth = isMoreMenu \? 880/,
  "shared more panel must use the wide card-grid portal",
);
assert.match(
  topbarMenuSrc,
  /event\.key === "Escape"[\s\S]*summary\.focus/,
  "shared topbar menus must close on Escape and restore focus",
);
assert.match(
  topbarMenuSrc,
  /ArrowLeft[\s\S]*topbar-more-card[\s\S]*\.focus/,
  "shared topbar menu must support keyboard movement inside the card grid",
);
assert.match(
  appShellCss,
  /\.topbar-more-grid[\s\S]*repeat\(auto-fit,[\s\S]*\.topbar-more-card/,
  "topbar more panel must render a responsive card grid",
);
assert.match(
  appShellCss,
  /\[data-mei-layout-fill="true"\][\s\S]*align-self:\s*stretch;[\s\S]*justify-self:\s*stretch;/,
  "common fill-down CSS must stretch both grid axes",
);

const modulesPath = path.join(root, "scripts", "build", "spa-navigation-modules.json");
const moduleList = JSON.parse(await readFile(modulesPath, "utf8"));
assert.ok(Array.isArray(moduleList) && moduleList.length > 0, "spa-navigation module list required");
assert.ok(
  moduleList.indexOf(objectSelectionModule) < moduleList.indexOf(mapWorldBridgeModule) &&
    moduleList.indexOf(objectSelectionModule) < moduleList.indexOf(focusControllerModule),
  "static module order must load object selection before map-world and focus bridges",
);

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
assert.match(
  src,
  /cross_app_full_navigation[\s\S]*location\.assign/,
  "cross-app topbar must full-page navigate (capabilities live in document head)",
);
assert.match(
  src,
  /a\.app-tab, a\.app-tab-sub[\s\S]*shouldBypassSpaClick|shouldBypassSpaClick[\s\S]*a\.app-tab, a\.app-tab-sub/,
  "cross-app app-tab clicks must bypass SPA click handling",
);
assert.match(
  src,
  /data-default-stage/,
  "topbar href fixer must honor per-app data-default-stage (not hardcode /home)",
);

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
  /function resolvePreviewAppId[\s\S]*?appIdFromAppsPathname/,
  "resolvePreviewAppId must prefer appIdFromAppsPathname for stage paths",
);

const ASSEMBLY_LINE_COUNT_TARGETS = [
  "spa-navigation/spa/structure-tree-materializer.js",
  "spa-navigation/spa/host-capabilities-ready.js",
  "spa-navigation/spa/view-assembly-coordinator.js",
];

for (const rel of ASSEMBLY_LINE_COUNT_TARGETS) {
  const chunk = await readFile(path.join(assetsRoot, rel), "utf8");
  const lines = chunk.split("\n").length;
  assert.ok(lines <= 540, `${rel} must stay within ~500 lines (got ${lines})`);
}

const structureAnchor = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/structure-anchor.js"),
  "utf8",
);
assert.match(structureAnchor, /data-mei-node-id/, "structure anchor must resolve mei node id");

const previewMaterializer = await readFile(
  path.join(assetsRoot, "spa-navigation/spa/preview-materializer.js"),
  "utf8",
);
assert.match(
  previewMaterializer,
  /function listStructureForPlane/,
  "preview materializer must expose listStructureForPlane",
);
assert.match(
  previewMaterializer,
  /data-build-node/,
  "compose DOM must stamp data-build-node for FAB structure focus",
);

const focusController = await readFile(
  path.join(assetsRoot, "spa-navigation/presentation/focus-controller.js"),
  "utf8",
);
assert.match(focusController, /focus_structure/, "focus controller must handle focus_structure");
assert.match(
  focusController,
  /mei:structure-focus/,
  "focusStructure must dispatch mei:structure-focus",
);
assert.match(focusController, /meiObjectId/, "focus targets must stamp objectId");
assert.match(focusController, /worldTarget\.objectId/, "world actions must preserve objectId");

const objectSelectionRuntime = await readFile(
  path.join(assetsRoot, objectSelectionModule),
  "utf8",
);
assert.match(objectSelectionRuntime, /mei:object-select/, "object select input event required");
assert.match(
  objectSelectionRuntime,
  /mei:object-selection-change/,
  "object selection change event required",
);
assert.doesNotMatch(
  objectSelectionRuntime,
  /query_state|queryState/,
  "object selection must stay separate from query_state",
);

const copilotToolbar = await readFile(
  path.join(assetsRoot, "spa-navigation/presentation/copilot-toolbar.js"),
  "utf8",
);
assert.match(
  copilotToolbar,
  /copilot-structure-picker/,
  "FAB toolbar must provide structure picker UI",
);

console.log("spa-navigation static checks ok");
console.log("view-assembly bundle checks ok");
console.log("phase-8.5 structure focus static checks ok");
