#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = fs.readFileSync(
  path.join(root, "app/assets/host-runtime-control-center.js"),
  "utf8",
);
const eventsSource = fs.readFileSync(path.join(root, "app/assets/host-runtime-events.js"), "utf8");
const devEvalSource = fs.readFileSync(
  path.join(root, "app/assets/spa-navigation/spa/dev-eval-scope.js"),
  "utf8",
);
const hub = fs.readFileSync(path.join(root, "host-shell/src/host_runtime_hub.rs"), "utf8");

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

for (const mount of [
  "data-runtime-profile-mount",
  "data-runtime-json-mount",
  "data-runtime-plan-mount",
  "data-runtime-dry-run-mount",
  "data-runtime-task-mount",
  "data-runtime-builds-mount",
  "data-runtime-cleanup-mount",
  "data-runtime-instances-mount",
  "data-runtime-manifest-mount",
]) {
  assert(hub.includes(mount), `runtime hub must expose ${mount}`);
}

assert(!hub.includes("<script>"), "runtime hub must not regress to an inline script");
assert(source.includes("expectedRevision: state.document.revision"), "save must use expectedRevision");
assert(source.includes("error.status === 409"), "save must preserve revision conflicts");
assert(source.includes('"/validate"') && source.includes('"/dry-run"'), "draft preview APIs must be used");
assert(source.includes('"/api/host/runtime/apply-profile"'), "apply-profile API must be used");
assert(source.includes('"/api/host/runtime/profile"'), "control-plane profile status API must be used");
assert(source.includes('"/api/host/launch-manifest"'), "launch-manifest API must be used");
assert(source.includes('"/api/host/instances"'), "instances API must be used");
assert(source.includes('"/api/host/builds/request"'), "builds/request API must be used");
assert(source.includes("build worker → launch instances → cutover route"), "apply copy must describe instance pipeline");
assert(
  source.includes("control?.selectedProfile?.id || \"default\""),
  "first boot must prefer the server-selected profile",
);
assert(
  hub.includes("data-runtime-control-status") && hub.includes("data-runtime-access-status"),
  "runtime hub must expose control and Access status",
);
assert(source.includes('"/api/host/builds"'), "workspace builds API must be used");
assert(
  source.includes('"/api/host/builds/cleanup-preview"') &&
    source.includes('"/api/host/builds/cleanup"'),
  "cleanup must require preview before execute",
);
assert(
  source.includes('import("/workspace-components/mei/overflow-text.js")'),
  "long generation text must reuse floating overflow text",
);
assert(
  !source.includes("global.location.reload()"),
  "generation lifecycle must refresh state without location.reload",
);
assert(
  source.includes("state.applyPreview = null") && !source.includes("await confirmProfileApply();"),
  "saving must not automatically apply a profile",
);
assert(!/\btitle\s*=/u.test(source), "long text must not use a title-only tooltip");
assert(hub.includes("实例与路由"), "runtime hub zone 02 must be instances/routes");
assert(hub.includes("Builder 任务"), "runtime hub zone 03 must be builder tasks");
assert(hub.includes("Bundle 与容量"), "runtime hub zone 04 must be bundle capacity");

const sandbox = {
  console,
  document: { readyState: "loading", addEventListener() {} },
  setTimeout,
  clearTimeout,
};
sandbox.window = sandbox;
sandbox.globalThis = sandbox;
vm.createContext(sandbox);
vm.runInContext(source, sandbox);

const api = sandbox.MeiHostRuntimeControlCenter;
assert(api, "control center helpers must be exported");
assert(api.validAppId("mini-data"), "dash app id should be valid");
assert(api.validAppId("*"), "wildcard app id should be valid");
assert(!api.validAppId("../escape"), "path-like app id should be invalid");
assert(
  api.errorMessage({
    error: {
      message: "workspace profile revision conflict",
      details: { currentRevision: "server-revision" },
    },
  }).includes("server-revision"),
  "conflict text must include the server revision",
);

const storage = new Map();
const eventsSandbox = {
  console,
  CustomEvent: class {
    constructor(type, init) {
      this.type = type;
      this.detail = init.detail;
    }
  },
  EventSource: undefined,
  location: {
    pathname: "/apps/mini-data/home",
    href: "http://localhost/apps/mini-data/home",
    reload() {},
  },
  sessionStorage: {
    getItem(key) {
      return storage.get(key) || null;
    },
    setItem(key, value) {
      storage.set(key, value);
    },
  },
  dispatchEvent() {},
  setTimeout,
};
eventsSandbox.window = eventsSandbox;
eventsSandbox.globalThis = eventsSandbox;
vm.createContext(eventsSandbox);
vm.runInContext(eventsSource, eventsSandbox);
const eventsApi = eventsSandbox.MeiHostRuntimeEvents;
assert(eventsApi.appliesToCurrentApp({ appId: "mini-data" }), "event should match current app");
assert(!eventsApi.appliesToCurrentApp({ appId: "other" }), "event should reject another app");
const applied = {
  appId: "mini-data",
  profileId: "local",
  profileRevision: "r1",
  revision: "mcg-r1",
};
assert(eventsApi.claimApplyEvent(applied), "first revision event should be claimed");
assert(!eventsApi.claimApplyEvent(applied), "duplicate revision event must be deduplicated");

const devEvalSandbox = {
  __mei: {
    dev_eval: {
      appId: "mini-data",
      runtimePlan: {
        defaultMode: "frozen",
        apps: {
          "mini-data": {
            targets: [
              { scope: "home/hot", mode: "hot" },
              { scope: "home/lazy", mode: "lazy" },
            ],
          },
        },
      },
    },
  },
};
devEvalSandbox.window = devEvalSandbox;
devEvalSandbox.globalThis = devEvalSandbox;
vm.createContext(devEvalSandbox);
vm.runInContext(devEvalSource, devEvalSandbox);
const devEval = devEvalSandbox.__meiLangBoot;
assert(devEval.devEvalAllowsWarmupScope("home/hot/panel"), "hot scope should warm");
assert(!devEval.devEvalAllowsWarmupScope("home/lazy/panel"), "lazy scope should not warm");
assert(devEval.devEvalAllowsEvalScope("home/lazy/panel"), "lazy scope should evaluate");
assert(!devEval.devEvalAllowsEvalScope("home/frozen"), "frozen scope should not evaluate");

console.log("host-runtime-control-center.test: ok");
