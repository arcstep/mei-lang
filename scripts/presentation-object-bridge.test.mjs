/**
 * Guard: viewpoint, map/world and presentation actions preserve explicit objectId.
 * Run: node scripts/presentation-object-bridge.test.mjs
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const assetsRoot = path.join(__dirname, "../host-shell/app/assets");

function extractFunction(source, name) {
  const start = source.indexOf(`function ${name}(`);
  assert.ok(start >= 0, `${name} not found`);
  let cursor = source.indexOf("{", start);
  let depth = 0;
  for (; cursor < source.length; cursor += 1) {
    if (source[cursor] === "{") depth += 1;
    if (source[cursor] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, cursor + 1);
    }
  }
  throw new Error(`unterminated ${name}`);
}

const focusSource = await readFile(
  path.join(assetsRoot, "spa-navigation/presentation/focus-controller.js"),
  "utf8",
);
const selected = [];
class HTMLElement {
  constructor() {
    this.dataset = {};
    this.classList = { add() {} };
  }
  scrollIntoView() {}
}
const focusTarget = new HTMLElement();
const focusContext = {
  CustomEvent: class CustomEvent {},
  CSS: { escape: (value) => value },
  HTMLElement,
  boot: {
    objectSelectionRuntime: {
      select(detail) {
        selected.push(detail);
      },
    },
  },
  clearViewpointFocus() {},
  readViewpointEntry() {
    return {
      tier: "t0",
      objectId: "domain.object-1",
      entityId: "entity-1",
    };
  },
  stampWorldTargetDataset(target, entry) {
    target.dataset.meiObjectId = entry.objectId;
  },
  document: {
    querySelector() {
      return focusTarget;
    },
    documentElement: { classList: { add() {} } },
  },
};
focusContext.window = focusContext;
focusContext.globalThis = focusContext;
vm.createContext(focusContext);
vm.runInContext(
  [
    extractFunction(focusSource, "syncObjectSelectionFromEntry"),
    extractFunction(focusSource, "focusViewpoint"),
    extractFunction(focusSource, "resolveWorldTarget"),
    "this.focusViewpoint = focusViewpoint;",
    "this.resolveWorldTarget = resolveWorldTarget;",
  ].join("\n"),
  focusContext,
);

assert.equal(focusContext.focusViewpoint("vp-object"), true);
assert.equal(selected.length, 1);
assert.equal(selected[0].objectId, "domain.object-1");
assert.equal(selected[0].mode, "replace");
assert.equal(focusTarget.dataset.meiObjectId, "domain.object-1");
focusContext.readViewpointEntry = () => ({ tier: "t0", entityId: "entity-only" });
assert.equal(focusContext.focusViewpoint("vp-legacy"), true);
assert.equal(selected.length, 1, "viewpoints without objectId must keep legacy focus behavior");

assert.equal(
  focusContext.resolveWorldTarget(
    { type: "focus_entity", object_id: "domain.explicit" },
    { objectId: "domain.entry" },
  ).objectId,
  "domain.explicit",
);
assert.equal(
  focusContext.resolveWorldTarget({ type: "focus_entity" }, { object_id: "domain.entry" })
    .objectId,
  "domain.entry",
);
assert.equal(
  Object.hasOwn(
    focusContext.resolveWorldTarget(
      { type: "focus_entity", entityId: "entity-only" },
      {},
    ),
    "objectId",
  ),
  false,
);

const bridgeSource = await readFile(
  path.join(assetsRoot, "spa-navigation/presentation/map-world-bridge.js"),
  "utf8",
);
let dispatchedAction = null;
class HTMLScriptElement {
  constructor(textContent) {
    this.textContent = textContent;
  }
}
const presentationMapNode = new HTMLScriptElement(
  JSON.stringify({
    viewpoints: {
      object_world_entry: {
        viewFamily: "world",
        objectId: "domain.object-1",
        entityId: "entity-1",
      },
      legacy_world_entry: {
        viewFamily: "world",
        entityId: "entity-legacy",
      },
    },
  }),
);
const bridgeContext = {
  console,
  HTMLScriptElement,
  document: {
    getElementById() {
      return presentationMapNode;
    },
  },
  boot: {
    dispatchPresentationAction(action) {
      dispatchedAction = action;
      return true;
    },
  },
};
vm.createContext(bridgeContext);
vm.runInContext(
  [
    extractFunction(bridgeSource, "readPresentationMap"),
    extractFunction(bridgeSource, "resolveWorldEntryViewpoint"),
    extractFunction(bridgeSource, "dispatchEnterWorldView"),
    "this.dispatchEnterWorldView = dispatchEnterWorldView;",
  ].join("\n"),
  bridgeContext,
);
assert.equal(
  bridgeContext.dispatchEnterWorldView({ object_id: "domain.object-1" }),
  true,
);
assert.equal(dispatchedAction.viewpoint, "object_world_entry");
assert.equal(dispatchedAction.objectId, "domain.object-1");

dispatchedAction = null;
bridgeContext.dispatchEnterWorldView({
  viewpoint: "object_world_entry",
  objectId: "domain.detail",
});
assert.equal(dispatchedAction.objectId, "domain.detail");

dispatchedAction = null;
bridgeContext.dispatchEnterWorldView({
  viewpoint: "legacy_world_entry",
  entityId: "entity-legacy",
});
assert.equal(Object.hasOwn(dispatchedAction, "objectId"), false);

const stepSource = await readFile(
  path.join(assetsRoot, "spa-navigation/presentation/presentation-step-engine.js"),
  "utf8",
);
const stepContext = {};
vm.createContext(stepContext);
vm.runInContext(
  `${extractFunction(stepSource, "normalizePresentationAction")}
this.normalizePresentationAction = normalizePresentationAction;`,
  stepContext,
);
assert.equal(
  stepContext.normalizePresentationAction({
    type: "focus_entity",
    object_id: "domain.step",
  }).objectId,
  "domain.step",
);
assert.equal(
  stepContext.normalizePresentationAction({
    type: "focus_entity",
    objectId: "domain.camel",
    object_id: "domain.snake",
  }).objectId,
  "domain.camel",
);

console.log("presentation-object-bridge.test.mjs: ok");
