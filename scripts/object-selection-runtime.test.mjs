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
context.addEventListener("mei:object-selection-change", (event) => {
  changes.push(event.detail);
});

assert.deepEqual(plain(api.selection.objectIds), []);

api.replace({
  objectIds: ["object-a", "object-b", "object-a"],
  primaryObjectId: "object-b",
  source: "test",
  secondary: { relation: "peer" },
});
assert.deepEqual(plain(api.getSelection()), {
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

api.replace({ entityId: "entity-only", source: "no-inference" });
assert.deepEqual(
  plain(api.getSelection().objectIds),
  [],
  "entityId must never be converted into an objectId",
);

api.clear({ source: "test-clear" });
assert.deepEqual(plain(api.getSelection()), {
  objectIds: [],
  primaryObjectId: "",
  source: "test-clear",
  mode: "clear",
});
assert.ok(changes.length >= 6, "each effective selection update must emit a change event");

console.log("object-selection-runtime.test.mjs: ok");
