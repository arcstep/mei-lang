/**
 * Guard: object selection state stays independent and supports all update modes.
 * Run: node scripts/object-selection-runtime.test.mjs
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const runtimePath = path.join(
  __dirname,
  "../host-shell/app/assets/spa-navigation/presentation/object-selection-runtime.js",
);
const source = await readFile(runtimePath, "utf8");
const fixture = JSON.parse(
  await readFile(
    path.join(__dirname, "../tests/fixtures/object-identity/runtime-index.json"),
    "utf8",
  ),
);
assert.doesNotMatch(source, /query_state|queryState/, "selection must stay outside query_state");

class RuntimeEvent {
  constructor(type, init = {}) {
    this.type = type;
    this.detail = init.detail;
  }
}

const listeners = new Map();
const context = {
  console,
  CustomEvent: RuntimeEvent,
  __mei: { presentation_map: { objectIndex: fixture.objectIndex } },
  addEventListener(type, listener) {
    const values = listeners.get(type) || [];
    values.push(listener);
    listeners.set(type, values);
  },
  dispatchEvent(event) {
    for (const listener of listeners.get(event.type) || []) listener(event);
    return true;
  },
};
context.window = context;
context.globalThis = context;
vm.createContext(context);
vm.runInContext(source, context);

const api = context.MeiObjectSelection;
assert.equal(api, context.__meiLangBoot.objectSelectionRuntime);
assert.equal(context.__meiLangBoot.bootObjectSelectionRuntime(), api);
const plain = (value) => JSON.parse(JSON.stringify(value));

const changes = [];
const interactionEvents = [];
const interactionDiagnostics = [];
let legacySelectEvents = 0;
context.addEventListener("mei:object-selection-change", (event) => {
  changes.push(event.detail);
});
context.addEventListener("mei:interaction-intent", (event) => {
  interactionEvents.push(event.detail);
});
context.addEventListener("mei:interaction-diagnostic", (event) => {
  interactionDiagnostics.push(event.detail);
});
context.addEventListener("mei:object-select", () => {
  legacySelectEvents += 1;
});

assert.deepEqual(plain(api.selection.objectIds), []);

api.replace({
  objectIds: ["object-a", "object-b", "object-a"],
  primaryObjectId: "object-b",
  source: "test",
  secondary: { relation: "peer" },
});
assert.deepEqual(plain(api.getSelection()), {
  objects: [
    { objectId: "object-a", identityStatus: "legacy" },
    { objectId: "object-b", identityStatus: "legacy" },
  ],
  objectIds: ["object-a", "object-b"],
  primaryObjectId: "object-b",
  source: "test",
  mode: "replace",
  secondary: { relation: "peer" },
});

api.add({ objectId: "object-c", source: "test-add" });
assert.deepEqual(plain(api.getSelection().objectIds), ["object-a", "object-b", "object-c"]);
assert.equal(api.getSelection().primaryObjectId, "object-b");

api.remove({ objectIds: ["object-b", "missing"], source: "test-remove" });
assert.deepEqual(plain(api.getSelection().objectIds), ["object-a", "object-c"]);
assert.equal(api.getSelection().primaryObjectId, "object-a");

context.dispatchEvent(
  new RuntimeEvent("mei:object-select", {
    detail: {
      object_id: "object-event",
      primary_object_id: "object-event",
      source: "event",
      mode: "replace",
    },
  }),
);
assert.deepEqual(plain(api.getSelection().objectIds), ["object-event"]);
assert.equal(api.getSelection().primaryObjectId, "object-event");
assert.equal(changes.at(-1).source, "event");

api.replace({
  objectType: fixture.objectType,
  objectKey: fixture.identityValue,
  source: "table-row",
});
const fromObjectKey = api.getSelection();
assert.equal(fromObjectKey.primaryObjectId, fixture.objectId);
assert.equal(fromObjectKey.objects[0].identityStatus, "canonical");
assert.equal(fromObjectKey.objects[0].objectType, fixture.objectType);

api.replace({
  objectType: fixture.objectType,
  entityId: fixture.identityValue,
  source: "map-feature",
});
assert.equal(
  api.getSelection().primaryObjectId,
  fromObjectKey.primaryObjectId,
  "table/map/world locators must resolve to the same canonical object",
);

api.replace({ entityId: "entity-only", source: "no-inference" });
assert.deepEqual(
  plain(api.getSelection().objectIds),
  [],
  "entityId must never be converted into an objectId",
);

const interactions = context.MeiInteraction;
assert.equal(interactions, context.__meiLangBoot.interactionRuntime);
const responderCalls = [];
interactions.registerResponder(
  {
    id: "detail",
    objectType: fixture.objectType,
    role: "detail",
    intents: ["select", "open_projection"],
  },
  (event) => responderCalls.push(event.intent),
);
interactions.registerResponder(
  {
    id: "chart",
    objectType: fixture.objectType,
    role: "chart",
    intents: ["explain_metric"],
  },
  (event) => responderCalls.push(event.intent),
);
interactions.registerResponder(
  {
    id: "map",
    objectType: fixture.objectType,
    role: "map",
    intents: ["focus_viewpoint"],
  },
  (event) => responderCalls.push(event.intent),
);
interactions.registerResponder(
  {
    id: "filter",
    objectType: fixture.objectType,
    role: "card",
    intents: ["filter_query"],
  },
  (event) => responderCalls.push(event.intent),
);

const focus = {
  objectType: fixture.objectType,
  objectKey: fixture.identityValue,
  source: "interaction-test",
};
interactions.dispatch("select", { ...focus, targetId: "detail" });
interactions.dispatch("open_projection", { ...focus, targetId: "detail" });
interactions.dispatch("focus_viewpoint", { ...focus, targetId: "map" });
interactions.dispatch("explain_metric", {
  objectType: fixture.objectType,
  metric: "warning_count",
  sourceRef: { kind: "dataset_ref", id: "warnings" },
  targetId: "chart",
});
interactions.dispatch("filter_query", {
  objectType: fixture.objectType,
  query: { severity: "high" },
  targetId: "filter",
});
assert.deepEqual(
  plain(interactionEvents.map((event) => event.intent)),
  ["select", "open_projection", "focus_viewpoint", "explain_metric", "filter_query"],
);
assert.deepEqual(plain(responderCalls), [
  "select",
  "open_projection",
  "focus_viewpoint",
  "explain_metric",
  "filter_query",
]);
assert.equal(interactionEvents[0].subject.kind, "object_focus");
assert.equal(interactionEvents[3].subject.kind, "object_set");
assert.equal(
  Object.hasOwn(interactions.getState().objectSet, "objectId"),
  false,
  "ObjectSet must never masquerade as objectId",
);
assert.equal(legacySelectEvents >= 2, true, "select keeps legacy + intent double fire");

let unsupportedCalls = 0;
interactions.registerResponder(
  {
    id: "other-type",
    objectType: "ops.Other",
    role: "detail",
    intents: ["open_projection"],
  },
  () => {
    unsupportedCalls += 1;
  },
);
interactions.dispatch("open_projection", { ...focus, targetId: "other-type" });
assert.equal(unsupportedCalls, 0, "unsupported object types quietly no-op");

let ambiguousCalls = 0;
interactions.registerResponder(
  {
    id: "detail-second",
    objectType: fixture.objectType,
    role: "detail",
    intents: ["open_projection"],
  },
  () => {
    ambiguousCalls += 1;
  },
);
interactions.dispatch("open_projection", focus);
assert.equal(ambiguousCalls, 0, "ambiguous responders must not be selected randomly");
assert.equal(interactionDiagnostics.at(-1).code, "responder_target_ambiguous");

const invalidSetCount = interactionEvents.length;
assert.equal(
  interactions.dispatch("explain_metric", {
    objectType: fixture.objectType,
    objectId: fixture.objectId,
    metric: "warning_count",
  }).subject,
  undefined,
);
assert.equal(interactionEvents.length, invalidSetCount + 1);
assert.equal(interactionDiagnostics.at(-1).code, "object_set_object_id_forbidden");

api.clear({ source: "test-clear" });
assert.deepEqual(plain(api.getSelection()), {
  objects: [],
  objectIds: [],
  primaryObjectId: "",
  source: "test-clear",
  mode: "clear",
});
assert.ok(changes.length >= 6, "each effective selection update must emit a change event");

console.log("object-selection-runtime.test.mjs: ok");
