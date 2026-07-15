#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const accessBundle = path.join(root, "host-shell/app/assets/dist/access.bundle.js");
const manageBundle = path.join(root, "host-shell/app/assets/dist/manage.bundle.js");

function assert(cond, msg) {
  if (!cond) {
    console.error(`FAIL: ${msg}`);
    process.exit(1);
  }
}

for (const bundlePath of [accessBundle, manageBundle]) {
  const code = fs.readFileSync(bundlePath, "utf8");
  assert(code.includes("layerStore"), `${bundlePath} should export layerStore`);
  assert(code.includes("viewCompositor"), `${bundlePath} should export viewCompositor`);
  assert(
    code.includes("sceneManifestLoader"),
    `${bundlePath} should export sceneManifestLoader`,
  );
  assert(
    code.includes("viewRevisionClient"),
    `${bundlePath} should export viewRevisionClient`,
  );
  assert(
    code.includes("layerArtifactCache"),
    `${bundlePath} should export layerArtifactCache`,
  );
}

const compositorSrc = fs.readFileSync(
  path.join(root, "host-shell/app/assets/spa-navigation/spa/view-compositor.js"),
  "utf8",
);
const g = { __meiLangBoot: {} };
const sandbox = { globalThis: g, window: g };
vm.createContext(sandbox);
vm.runInContext(
  compositorSrc.replace(
    /\(typeof window !== "undefined" \? window : globalThis\)/,
    "(globalThis)",
  ),
  sandbox,
);
const compositor = g.__meiLangBoot?.viewCompositor;
assert(compositor, "viewCompositor should initialize");
const nodes = compositor.nodesForProjection(
  {
    nodes: [
      { ui_role: "region", preview_scope: "r1" },
      { ui_role: "content", preview_scope: "c1" },
    ],
  },
  "plane_region",
);
assert(nodes.length === 1, "plane_region should hide content nodes");
assert(
  typeof compositor.composeFromLayers === "function",
  "composeFromLayers should be exported",
);

console.log("test-artifact-layers-static: ok");
