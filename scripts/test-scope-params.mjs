import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readFile } from "node:fs/promises";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const scopePath = path.join(root, "app", "assets", "scope-params.js");

// scope-params.js 为浏览器 IIFE；用 vm 执行后挂到 globalThis
const code = await readFile(scopePath, "utf8");
const vm = await import("node:vm");
vm.runInThisContext(code, { filename: "scope-params.js" });

const { MeiAgentScopeParams } = globalThis;
assert.ok(MeiAgentScopeParams, "MeiAgentScopeParams should be defined");

assert.equal(
  MeiAgentScopeParams.defaultResourceVisibilityFromRoute("access", "ask"),
  "allow_scene_reachable",
);
assert.equal(
  MeiAgentScopeParams.defaultResourceVisibilityFromRoute("manage", "ask"),
  "allow_direct_refs",
);
assert.equal(
  MeiAgentScopeParams.defaultResourceVisibilityFromRoute("manage", "build"),
  "allow_direct_refs",
);
assert.equal(
  MeiAgentScopeParams.defaultResourceVisibilityFromRoute("access", "build"),
  "local_only",
);

assert.equal(
  MeiAgentScopeParams.effectiveResourceVisibility("allow_scene_reachable", "manage", "ask"),
  "allow_scene_reachable",
);
assert.equal(
  MeiAgentScopeParams.effectiveResourceVisibility("", "manage", "ask"),
  "allow_direct_refs",
);

assert.equal(MeiAgentScopeParams.normTargetKeyForScope(".\\foo\\bar"), "foo/bar");
assert.equal(
  MeiAgentScopeParams.shouldAttachSceneIdToScopeQuery("", "scene/main.mei"),
  true,
);
assert.equal(
  MeiAgentScopeParams.shouldAttachSceneIdToScopeQuery("data/x", "scene/main.mei"),
  false,
);
assert.equal(
  MeiAgentScopeParams.shouldAttachSceneIdToScopeQuery("scene/main.mei", "scene/main.mei"),
  true,
);

const q = MeiAgentScopeParams.scopeQueryCore(
  "demo/app",
  "main.mei",
  "manage",
  "ask",
  "allow_direct_refs",
);
assert.equal(q.get("resource_visibility"), "allow_direct_refs");
assert.equal(q.get("route_mode"), "manage");
assert.equal(q.get("mode"), "ask");

console.log("scope-params tests ok");
