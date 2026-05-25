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

console.log("spa-navigation static checks ok");
