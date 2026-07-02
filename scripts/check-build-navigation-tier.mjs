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
const navP1 = readFileSync(path.join(root, "app/assets/build-navigation/p1.js"), "utf8");
const navP2 = readFileSync(path.join(root, "app/assets/build-navigation/p2.js"), "utf8");
const src = `${navP1}\n${navP2}`;

assert.match(src, /classifyBuildNavTier/, "tier classifier");
assert.match(src, /shouldSkipPreviewRuntimeWake/, "runtime wake bypass");
assert.match(src, /workspace-fragment/, "tier1 fragment fetch");
assert.match(src, /__meiBuildNavStats/, "nav stats");
assert.match(src, /inferPreviewTabFromNodeId/, "node-aware preview tab inference");
assert.match(src, /ui-scope:/, "ui-scope preview tab inference");
assert.match(src, /isSameSceneStructureNav/, "same-scene structure tier0");
assert.match(src, /readUiScopeMetaFromReachabilityTree/, "ui-scope tier0 DOM check");
assert.match(src, /isPackCatalogNodeId/, "pack catalog runtime reset");
assert.match(src, /ensurePreviewTabVisible\(url\)/, "preview tab before fragment swap");
assert.match(src, /__meiBuildNavLastTier/, "tier debug probe");
assert.match(src, /structureNav/, "structure nav tier0 force");
assert.doesNotMatch(src, /isBuildCatalogPreviewNode\(nextNode\)\) return "fragment"/, "catalog uses coordinate-based tier");

console.log("check-build-navigation-tier: ok");
