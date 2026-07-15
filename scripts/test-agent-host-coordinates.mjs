import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readFile } from "node:fs/promises";
import vm from "node:vm";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const scopePath = path.join(root, "host-shell", "app", "assets", "scope-params.js");
const hostPath = path.join(root, "host-shell", "app", "assets", "agent-host-coordinates.js");

const scopeCode = await readFile(scopePath, "utf8");
const hostCode = await readFile(hostPath, "utf8");

globalThis.document = {
  getElementById(id) {
    if (id === "author-resource-visibility-select") {
      return { value: "" };
    }
    return null;
  },
};

vm.runInThisContext(scopeCode, { filename: "scope-params.js" });
vm.runInThisContext(hostCode, { filename: "agent-host-coordinates.js" });

assert.ok(globalThis.MeiAgentHostCoordinates, "MeiAgentHostCoordinates should be defined");

const browserContext = {
  schema: "access_browser_context_v1",
  active_query_state_ids: ["q_home"],
  query_states: [{ id: "q_home", filters: { region: "华东" } }],
};

const api = {
  root: {
    dataset: {
      mode: "app",
      app: "examples/ds/01-dataset-baseline",
    },
  },
  state: { agentMode: "ask" },
  normalizeRouteMode(value) {
    const mode = String(value || "").trim().toLowerCase();
    if (mode === "access" || mode === "app" || mode === "run") return "access";
    return "manage";
  },
  normalizeAgentMode(value) {
    return String(value || "").trim().toLowerCase() === "ask" ? "ask" : "build";
  },
  currentAppKey() {
    return "examples/ds/01-dataset-baseline";
  },
  currentSceneId() {
    return "home";
  },
  currentTargetKey() {
    return "main.mei";
  },
  collectBrowserContext() {
    return browserContext;
  },
};

const body = globalThis.MeiAgentHostCoordinates.buildPromptRequestBody(api, "查看今日指标");
const params = globalThis.MeiAgentHostCoordinates.applyToUrlSearchParams(
  new URLSearchParams(),
  globalThis.MeiAgentHostCoordinates.build(api),
);

assert.equal(body.route_mode, "access");
assert.equal(body.mode, "ask");
assert.equal(body.scene_id, "home");
assert.deepEqual(body.browser_context, browserContext);
assert.equal(body.host_protocol.schema, "mei-host-runtime-protocol-v1");
assert.equal(body.host_protocol.surface, "access_host");
assert.equal(body.host_contract_schema, "mei-host-runtime-contract-v1");
assert.equal(params.get("route_mode"), "access");
assert.equal(params.get("scene_id"), "home");
assert.ok(params.get("host_protocol"), "host_protocol query param should exist");
assert.equal(params.get("host_contract_schema"), "mei-host-runtime-contract-v1");

console.log("agent-host-coordinates tests ok");
