#!/usr/bin/env node
/**
 * Headless helper: classify build navigation tiers from compile coordinates on links.
 * Usage: node scripts/check-build-navigation-tier.mjs
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const src = readFileSync(path.join(root, "app/assets/build-navigation.js"), "utf8");

assert.match(src, /classifyBuildNavTier/, "tier classifier");
assert.match(src, /shouldSkipPreviewRuntimeWake/, "runtime wake bypass");
assert.match(src, /workspace-fragment/, "tier1 fragment fetch");
assert.match(src, /__meiBuildNavStats/, "nav stats");

console.log("check-build-navigation-tier: ok");
