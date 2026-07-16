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
const fixture = JSON.parse(
  await readFile(
    path.join(__dirname, "../tests/fixtures/object-identity/runtime-index.json"),
    "utf8",
  ),
);
const canonicalDescriptor = fixture.objectIndex.descriptors[fixture.objectId];
const plain = (value) => JSON.parse(JSON.stringify(value));

function resolveFixtureObject(input) {
  const objectId = String(input?.objectId || input?.object_id || "");
  const objectType = String(input?.objectType || input?.object_type || "");
  const objectKey = input?.objectKey ?? input?.object_key;
  const entityId = input?.entityId ?? input?.entity_id;
  if (
    objectId === fixture.objectId ||
    (objectType === fixture.objectType &&
      (objectKey === fixture.identityValue || entityId === fixture.identityValue))
  ) {
    return { ...canonicalDescriptor, identityStatus: "canonical" };
  }
  if (objectId) return { objectId, identityStatus: "legacy" };
  return null;
}

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
    objectResolver: { resolve: resolveFixtureObject },
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
      objectId: fixture.objectId,
      objectType: fixture.objectType,
      objectKey: fixture.identityValue,
      entityId: fixture.identityValue,
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
assert.equal(selected[0].descriptor.objectId, fixture.objectId);
assert.equal(selected[0].descriptor.identityStatus, "canonical");
assert.equal(selected[0].mode, "replace");
assert.equal(focusTarget.dataset.meiObjectId, fixture.objectId);
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
const pickIntents = [];
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
        objectId: fixture.objectId,
        objectType: fixture.objectType,
        objectKey: fixture.identityValue,
        entityId: fixture.identityValue,
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
    objectResolver: { resolve: resolveFixtureObject },
    interactionRuntime: {
      dispatchMany(intents, detail) {
        pickIntents.push({ intents, detail });
        return intents.map((intent) => ({ intent }));
      },
    },
    dispatchPresentationAction(action) {
      dispatchedAction = action;
      return true;
    },
  },
};
bridgeContext.window = bridgeContext;
vm.createContext(bridgeContext);
vm.runInContext(
  [
    extractFunction(bridgeSource, "readPresentationMap"),
    extractFunction(bridgeSource, "resolveWorldEntryViewpoint"),
    extractFunction(bridgeSource, "dispatchEnterWorldView"),
    extractFunction(bridgeSource, "dispatchMapWorldObjectPick"),
    "this.dispatchEnterWorldView = dispatchEnterWorldView;",
    "this.dispatchMapWorldObjectPick = dispatchMapWorldObjectPick;",
  ].join("\n"),
  bridgeContext,
);
assert.equal(
  bridgeContext.dispatchEnterWorldView({ object_id: fixture.objectId }),
  true,
);
assert.equal(dispatchedAction.viewpoint, "object_world_entry");
assert.equal(dispatchedAction.objectId, fixture.objectId);
assert.equal(dispatchedAction.objectDescriptor.objectType, fixture.objectType);

dispatchedAction = null;
bridgeContext.dispatchEnterWorldView({
  viewpoint: "object_world_entry",
  objectType: fixture.objectType,
  objectKey: fixture.identityValue,
});
assert.equal(dispatchedAction.objectId, fixture.objectId);

dispatchedAction = null;
bridgeContext.dispatchEnterWorldView({
  viewpoint: "legacy_world_entry",
  entityId: "entity-legacy",
});
assert.equal(Object.hasOwn(dispatchedAction, "objectId"), false);

assert.equal(
  bridgeContext.dispatchMapWorldObjectPick({
    objectType: fixture.objectType,
    objectKey: fixture.identityValue,
  }),
  true,
);
assert.deepEqual(plain(pickIntents[0].intents), ["select", "focus_viewpoint"]);
assert.equal(
  pickIntents[0].intents.includes("open_projection"),
  false,
  "map/world pick must never implicitly open T2",
);

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
