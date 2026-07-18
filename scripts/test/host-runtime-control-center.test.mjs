#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const source = fs.readFileSync(
  path.join(root, "host-shell/app/assets/host-runtime-control-center.js"),
  "utf8",
);
const eventsSource = fs.readFileSync(path.join(root, "host-shell/app/assets/host-runtime-events.js"), "utf8");
const devEvalSource = fs.readFileSync(
  path.join(root, "host-shell/app/assets/spa-navigation/spa/dev-eval-scope.js"),
  "utf8",
);
const hub = fs.readFileSync(path.join(root, "host-shell/src/host_runtime_hub.rs"), "utf8");

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

for (const mount of [
  "data-host-runtime-control-center",
  "data-runtime-app-grid",
  "data-runtime-global-ops",
  "data-runtime-live",
  "data-runtime-cleanup-modal",
  "data-runtime-refresh-instances",
]) {
  assert(hub.includes(mount), `runtime hub must expose ${mount}`);
}

assert(!/<div[^>]*data-runtime-profile-mount\b/.test(hub), "legacy profile mount must be removed");
assert(!hub.includes(">配置档 / 启动清单<"), "legacy profile zone must be removed");
assert(!hub.includes("<script>"), "runtime hub must not regress to an inline script");
assert(!hub.includes("运行控制中心</h1>"), "runtime hub must not show legacy hero title");
assert(!hub.includes("按应用选择 launch"), "runtime hub must not show legacy hero blurb");
assert(!hub.includes("工具链 <code>"), "runtime hub must not show toolchain status strip");
assert(hub.includes('aria-label="应用"'), "runtime hub must expose apps section");

assert(source.includes('"/api/host/apps"'), "control center must call apps API");
assert(!source.includes('"/api/host/runtime/profile"'), "card hub must not depend on control-plane profile strip");
assert(source.includes('"/api/host/ops/prebuild"'), "compile-and-load must use ops prebuild");
assert(
  source.includes('"/api/host/builds/cleanup-preview"') &&
    source.includes('"/api/host/builds/cleanup"'),
  "cleanup must require preview before execute",
);
assert(source.includes("appId="), "cleanup preview must scope by appId");
assert(source.includes('phase === "ready"'), "enter link only when app runtime is ready");
assert(
  source.includes("mei-runtime-control__enter"),
  "running cards still expose enter when ready",
);
assert(source.includes("renderCleanupModal"), "cleanup must use temporary modal");
assert(source.includes("closeCleanupModal"), "cleanup modal must be closable without grid rerender");
assert(!source.includes("data-runtime-cleanup-inline"), "cleanup must not expand cards inline");
assert(
  source.includes('import("/workspace-components/mei/overflow-text.js")'),
  "long generation text must reuse floating overflow text",
);
assert(!source.includes("global.location.reload()"), "must refresh state without location.reload");
assert(
  !source.includes('title="') || source.includes("data-runtime-locked"),
  "button titles are ok for disabled reasons; overflow still uses floating popover",
);
assert(!source.includes("/api/host/workspace-profiles"), "card hub must not depend on workspace-profiles UI");
assert(source.includes("data-runtime-locked"), "buttons must encode availability locks");
assert(source.includes("hasCurrentBundle"), "start must require current compile artifact");
assert(source.includes("data-runtime-mode-select"), "must expose hot/lazy/frozen mode select for start/reload");
assert(!source.includes("data-runtime-mode-apply"), "must not expose separate 应用模式 action");
assert(!source.includes("data-runtime-mode-reset"), "must not expose 恢复默认 / overlay reset action");
assert(!source.includes("应用模式"), "must not show 应用模式 button label");
assert(!source.includes("恢复默认"), "must not show 恢复默认 button label");
assert(!source.includes("恢复 Git"), "must not show 恢复 Git button label");
assert(!source.includes("data-runtime-launch-select"), "multi-launch select must be removed");
assert(!source.includes("运行策略"), "must not repeat launch strategy blurb on every card");
assert(source.includes("跟随 launch.json"), "mode select must allow follow launch.json default");
assert(source.includes("startBodyForApp"), "start/reload must read selected mode");

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
assert(api.formatDuration(65000).includes("1m"), "duration helper should format minutes");

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
assert(eventsApi.shellNavFromLocation() === "", "access path has no workspace shellNav");
eventsSandbox.location.pathname = "/runtime";
assert(eventsApi.shellNavFromLocation() === "runtime", "runtime path maps shellNav");
eventsSandbox.location.pathname = "/apps/mini-data/home";

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
