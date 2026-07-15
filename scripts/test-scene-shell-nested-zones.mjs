/**
 * Regression: nested list_preview shell zones (ai_warning_cockpit_board) must survive layout filtering.
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const assetsRoot = path.join(root, "host-shell", "app", "assets");

function nonEmptyString(...values) {
  for (const value of values) {
    const text = String(value || "").trim();
    if (text) return text;
  }
  return "";
}

function boolValue(...values) {
  for (const value of values) {
    if (typeof value === "boolean") return value;
  }
  return undefined;
}

function positiveInt(...values) {
  for (const value of values) {
    const parsed = Number(value);
    if (Number.isFinite(parsed) && parsed > 0) {
      return Math.floor(parsed);
    }
  }
  return 0;
}

const primitivesSrc = await readFile(path.join(assetsRoot, "spa-navigation", "primitives.js"), "utf8");
const sceneShellSrc = await readFile(
  path.join(assetsRoot, "spa-navigation", "drilldown", "scene-shell-normalize.js"),
  "utf8",
);

const sandbox = {
  nonEmptyString,
  boolValue,
  positiveInt,
  exports: {},
};
vm.createContext(sandbox);
vm.runInContext(
  `${primitivesSrc}\n${sceneShellSrc}\nexports = {\n  retainZonesMatchingLayout,\n  normalizeSceneShellContract,\n  inferSceneShellLayoutMode,\n};`,
  sandbox,
);

const { retainZonesMatchingLayout, normalizeSceneShellContract, inferSceneShellLayoutMode } =
  sandbox.exports;

const aiWarningZones = [
  {
    id: "left",
    role: "container",
    area: "left",
    layout: {
      areas: [["chart"], ["detail"]],
    },
  },
  {
    id: "chart",
    role: "slots",
    area: "chart",
    parent: "left",
    accepts: ["chart"],
  },
  {
    id: "detail",
    role: "slots",
    area: "detail",
    parent: "left",
    accepts: ["data_table"],
    required: true,
  },
  {
    id: "preview",
    role: "row_preview",
    area: "preview",
    accepts: ["summary"],
    selectionSource: "detail",
  },
];

const aiWarningLayout = {
  areas: [["left", "preview"]],
};

const filteredAi = retainZonesMatchingLayout(aiWarningLayout, aiWarningZones);
const filteredIds = filteredAi.map((zone) => zone.id).sort();
assert.deepEqual(filteredIds, ["chart", "detail", "left", "preview"], "nested chart/detail zones kept");

const slotRoles = filteredAi.filter((zone) => zone.role === "slots");
assert.equal(slotRoles.length, 2, "two slot zones for chart + detail");

const shell = normalizeSceneShellContract(null, null, {
  layout_mode: "list_preview",
  layout: aiWarningLayout,
  zones: aiWarningZones,
});
assert.equal(shell.layoutMode, "list_preview");
assert.equal(shell.zones.filter((zone) => zone.role === "slots").length, 2);

const analyticsZones = [
  { id: "filter", role: "filter", area: "filter" },
  { id: "chart", role: "slots", area: "chart", accepts: ["chart"] },
  { id: "detail", role: "slots", area: "detail", accepts: ["data_table"], required: true },
];
const analyticsLayout = {
  areas: [
    ["filter", "chart"],
    ["filter", "detail"],
  ],
};
const filteredAnalytics = retainZonesMatchingLayout(analyticsLayout, analyticsZones);
assert.deepEqual(
  filteredAnalytics.map((zone) => zone.id).sort(),
  ["chart", "detail", "filter"],
  "flat analytics shell unchanged",
);
assert.equal(inferSceneShellLayoutMode(filteredAnalytics), "analytics");

console.log("scene-shell nested zones checks ok");
