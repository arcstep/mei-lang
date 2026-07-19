import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const source = await readFile(
  path.join(
    root,
    "host-shell",
    "app",
    "assets",
    "spa-navigation",
    "visit-history-store.js",
  ),
  "utf8",
);

function loadStore(href, appTitle, stageTitle = "", stageProfile = "") {
  const values = new Map();
  const location = new URL(href);
  const document = {
    body: { dataset: {} },
    querySelector(selector) {
      const metaMatch = selector.match(/^meta\[name="([^"]+)"\]$/);
      if (metaMatch) {
        const meta = {
          "mei-app-short-title": appTitle,
          "mei-app-title": appTitle,
          "mei-stage-short-title": stageTitle,
          "mei-stage-profile": stageProfile,
          "mei-workspace-label": "测试工作区",
          "mei-auth-user": "tester",
        };
        const value = meta[metaMatch[1]];
        return value == null ? null : { getAttribute: () => value };
      }
      return null;
    },
    dispatchEvent() {},
  };
  const context = {
    URL,
    location,
    document,
    localStorage: {
      getItem(key) {
        return values.get(key) ?? null;
      },
      setItem(key, value) {
        values.set(key, value);
      },
      removeItem(key) {
        values.delete(key);
      },
    },
    CustomEvent: class CustomEvent {
      constructor(type, options) {
        this.type = type;
        this.detail = options?.detail;
      }
    },
    MeiRoutePredicates: {
      appIdFromAppsPathname(pathname) {
        const parts = String(pathname).split("/").filter(Boolean);
        return parts[0] === "apps" ? parts[1] || "" : "";
      },
      sceneIdFromPathname(pathname) {
        const parts = String(pathname).split("/").filter(Boolean);
        return parts[0] === "apps" ? parts[2] || "" : "";
      },
    },
  };
  context.window = context;
  context.globalThis = context;
  vm.runInNewContext(source, context, { filename: "visit-history-store.js" });
  return context.MeiVisitHistoryStore;
}

{
  const store = loadStore(
    "http://localhost/apps/mini-data/supervision?chrome=none",
    "迷你数据",
    "监督",
    "slides",
  );
  const ctx = store.collectVisitContext();
  assert.equal(ctx.appId, "mini-data");
  assert.equal(ctx.scene, "supervision");
  assert.equal(ctx.routeKind, "stage");
  assert.equal(ctx.independent, true);
  assert.match(ctx.routeLabel, /迷你数据 · 监督 · 幻灯片 · 独立打开/);
}

{
  const store = loadStore(
    "http://localhost/admin/apps/mini-data/dataset/source",
    "迷你数据",
  );
  const ctx = store.collectVisitContext();
  assert.equal(ctx.appId, "mini-data");
  assert.equal(ctx.routeKind, "admin");
  assert.equal(ctx.resource, "dataset");
  assert.equal(ctx.module, "source");
  assert.match(ctx.routeLabel, /迷你数据 · source · 应用管理/);
}

console.log("visit-history-store tests passed");
