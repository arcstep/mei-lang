import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const assetsRoot = path.join(root, "host-shell", "app", "assets", "spa-navigation");

async function testClientErrorTraceAndDedupe() {
  const requests = [];
  const timers = [];
  const document = {
    body: { dataset: {}, getAttribute: () => null },
    documentElement: { dataset: {} },
    readyState: "complete",
    addEventListener() {},
    querySelector() {
      return null;
    },
  };
  const window = {
    __meiLangBoot: {},
    __mei: {},
    location: {
      href: "http://127.0.0.1:9527/apps/zhifa/home",
      pathname: "/apps/zhifa/home",
      search: "",
    },
    document,
    console: { error() {}, warn() {}, log() {}, info() {} },
    performance: {
      now: () => 1,
      getEntriesByType: () => [],
    },
    localStorage: { getItem: () => null },
    fetch: async (url, init) => {
      requests.push({ url, init });
      return {
        ok: true,
        status: 204,
        statusText: "",
        headers: { get: () => null },
      };
    },
    addEventListener() {},
    setTimeout(callback) {
      timers.push(callback);
      return timers.length;
    },
    clearTimeout() {},
    setInterval() {
      return 1;
    },
    clearInterval() {},
  };
  window.window = window;
  window.globalThis = window;

  const source = await readFile(
    path.join(assetsRoot, "spa", "render-pipeline-timeline.js"),
    "utf8",
  );
  vm.runInNewContext(source, {
    window,
    globalThis: window,
    URL,
    Date,
    Math,
    JSON,
    String,
    Number,
    Error,
    Promise,
    setTimeout: window.setTimeout,
  });

  const input = {
    kind: "drilldown_error",
    message: "看板加载失败",
    sceneId: "warnings_analytics_page",
    phase: "structured_slot_mount_failed",
  };
  const firstTrace = window.__meiLangBoot.reportClientError(input);
  const secondTrace = window.__meiLangBoot.reportClientError(input);
  assert.match(firstTrace, /^client-error-/);
  assert.equal(secondTrace, firstTrace, "deduped failures must keep the same trace id");
  assert.equal(requests.length, 1, "duplicate failures must not immediately spam Host");

  const firstPayload = JSON.parse(requests[0].init.body);
  assert.equal(firstPayload.id, firstTrace);
  assert.equal(firstPayload.detail.traceId, firstTrace);
  assert.equal(firstPayload.detail.sceneId, "warnings_analytics_page");

  assert.equal(timers.length, 1, "a repeat summary must be scheduled");
  timers[0]();
  assert.equal(requests.length, 2, "a repeated burst must emit one aggregate summary");
  const repeatPayload = JSON.parse(requests[1].init.body);
  assert.equal(repeatPayload.id, firstTrace);
  assert.equal(repeatPayload.detail.occurrenceCount, 2);
  assert.ok(repeatPayload.detail.firstOccurredAt);
  assert.ok(repeatPayload.detail.lastOccurredAt);
}

async function testPopupIssueBridgesToHost() {
  class HTMLElement {
    constructor() {
      this.dataset = {};
    }
  }
  const reports = [];
  const rootElement = new HTMLElement();
  const context = {
    HTMLElement,
    window: null,
    document: { getElementById: () => null },
    boot: {
      reportClientError(detail) {
        reports.push(detail);
        return "client-error-popup";
      },
    },
    nonEmptyString: (...values) =>
      values.map((value) => String(value || "").trim()).find(Boolean) || "",
    console: { error() {}, warn() {} },
  };
  context.window = context;
  const source = await readFile(path.join(assetsRoot, "drilldown", "debug.js"), "utf8");
  vm.runInNewContext(source, context);
  const traceId = context.recordPopupDebugIssue({
    message: "projection slots 为空",
    phase: "structured_slot_mount_failed",
    detail: { scene_id: "warnings_analytics_page" },
    config: {},
    root: rootElement,
  });

  assert.equal(traceId, "client-error-popup");
  assert.equal(rootElement.dataset.meiClientErrorTraceId, "client-error-popup");
  assert.equal(reports.length, 1);
  assert.equal(reports[0].kind, "drilldown_error");
  assert.equal(reports[0].sceneId, "warnings_analytics_page");
}

async function testVisibleErrorContainsTraceId() {
  class HTMLElement {
    constructor() {
      this.dataset = {};
      this.textContent = "";
      this.hidden = false;
    }

    toggleAttribute(name, force) {
      if (name === "hidden") this.hidden = force;
    }
  }
  const errorNode = new HTMLElement();
  errorNode.dataset.drilldownStatus = "error";
  errorNode.textContent = "看板加载失败，请稍后重试。";
  const rootElement = new HTMLElement();
  rootElement.querySelectorAll = (selector) =>
    selector === '[data-drilldown-status="error"]' || selector === "[data-drilldown-status]"
      ? [errorNode]
      : [];
  const context = {
    HTMLElement,
    boot: {},
    recordPopupDebugIssue() {
      rootElement.dataset.meiClientErrorTraceId = "client-error-visible";
      return "client-error-visible";
    },
    document: {
      getElementById: () => null,
      body: { appendChild() {}, classList: { remove() {} } },
      addEventListener() {},
    },
    DRILLDOWN_OVERLAY_ROOT_ID: "access-drilldown-overlay-root",
    SCENE_BOARD_OVERLAY_ROOT_ID: "access-scene-board-overlay-root",
  };
  const source = await readFile(
    path.join(assetsRoot, "drilldown", "overlay-chrome.js"),
    "utf8",
  );
  vm.runInNewContext(source, context);
  const traceId = context.setDrilldownOverlayStatus(rootElement, "error", {
    message: "结构化看板挂载失败",
    phase: "structured_mount_failed",
  });

  assert.equal(traceId, "client-error-visible");
  assert.match(errorNode.textContent, /追踪编号：client-error-visible/);
  assert.equal(errorNode.hidden, false);
}

await testClientErrorTraceAndDedupe();
await testPopupIssueBridgesToHost();
await testVisibleErrorContainsTraceId();

console.log("client error reporting checks ok");
