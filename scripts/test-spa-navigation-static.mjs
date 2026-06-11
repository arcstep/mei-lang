/**
 * 静态检查 spa-navigation.js：防止再次出现 navigate 未定义等回归。
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const spaPath = path.join(root, "app", "assets", "spa-navigation.js");
const src = await readFile(spaPath, "utf8");

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
assert.match(src, /shouldMountDrilldownHost/, "drilldown host gate");
assert.match(
  src,
  /function resolvePreviewAppId[\s\S]*?appRouteSlugFromPathname/,
  "resolvePreviewAppId must derive app id from route slug",
);

console.log("spa-navigation static checks ok");
