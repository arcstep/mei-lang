#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = fs.readFileSync(
  path.join(root, "host-shell/app/assets/host-runtime-events.js"),
  "utf8",
);

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function wait(ms = 0) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

class MockEventTarget {
  constructor() {
    this.listeners = new Map();
  }

  addEventListener(type, listener) {
    const listeners = this.listeners.get(type) || [];
    listeners.push(listener);
    this.listeners.set(type, listeners);
  }

  dispatchEvent(event) {
    for (const listener of this.listeners.get(event.type) || []) {
      listener(event);
    }
    return true;
  }
}

class MockLockManager {
  constructor() {
    this.holder = false;
  }

  request(name, _options, callback) {
    if (this.holder) return Promise.resolve(callback(null));
    this.holder = true;
    return Promise.resolve(callback({ name })).finally(() => {
      this.holder = false;
    });
  }
}

class MockBroadcastHub {
  constructor() {
    this.channels = new Set();
  }

  channelClass() {
    const hub = this;
    return class {
      constructor(name) {
        this.name = name;
        this.listeners = [];
        hub.channels.add(this);
      }

      addEventListener(type, listener) {
        if (type === "message") this.listeners.push(listener);
      }

      postMessage(data) {
        for (const channel of hub.channels) {
          if (channel === this || channel.name !== this.name) continue;
          for (const listener of channel.listeners) listener({ data });
        }
      }

      close() {
        hub.channels.delete(this);
      }
    };
  }
}

class MockEventSource {
  static instances = [];

  constructor(url) {
    this.url = url;
    this.closed = false;
    this.listeners = new Map();
    MockEventSource.instances.push(this);
  }

  addEventListener(type, listener) {
    this.listeners.set(type, listener);
  }

  emit(type, payload) {
    this.listeners.get(type)?.({
      data: JSON.stringify({ type, emittedAtMs: Date.now(), payload }),
    });
  }

  close() {
    this.closed = true;
  }
}

function createStorage() {
  const values = new Map();
  return {
    getItem(key) {
      return values.get(key) || null;
    },
    setItem(key, value) {
      values.set(key, String(value));
    },
    removeItem(key) {
      values.delete(key);
    },
  };
}

let tabSequence = 0;

function createTab({
  locks,
  BroadcastChannel,
  localStorage = createStorage(),
  visibilityState = "visible",
  focused = true,
} = {}) {
  const globalEvents = new MockEventTarget();
  const documentEvents = new MockEventTarget();
  const received = [];
  const document = {
    visibilityState,
    body: { getAttribute() { return ""; } },
    getElementById() {
      return null;
    },
    hasFocus() {
      return focused;
    },
    addEventListener: documentEvents.addEventListener.bind(documentEvents),
    dispatchEvent: documentEvents.dispatchEvent.bind(documentEvents),
  };
  const sandbox = {
    console,
    CustomEvent: class {
      constructor(type, init) {
        this.type = type;
        this.detail = init?.detail;
      }
    },
    EventSource: MockEventSource,
    BroadcastChannel,
    URLSearchParams,
    crypto: { randomUUID: () => `tab-${++tabSequence}` },
    navigator: locks ? { locks } : {},
    localStorage,
    sessionStorage: createStorage(),
    location: {
      pathname: "/runtime",
      search: "",
      href: "http://localhost/runtime",
      reload() {},
    },
    document,
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    fetch: async () => ({
      ok: true,
      json: async () => ({ digest: "d1", runningAppIds: [] }),
    }),
    addEventListener: globalEvents.addEventListener.bind(globalEvents),
    dispatchEvent(event) {
      received.push(event);
      return globalEvents.dispatchEvent(event);
    },
  };
  sandbox.window = sandbox;
  sandbox.globalThis = sandbox;
  vm.createContext(sandbox);
  vm.runInContext(source, sandbox);
  return {
    sandbox,
    received,
    setVisibility(next) {
      document.visibilityState = next;
      documentEvents.dispatchEvent({ type: "visibilitychange" });
    },
  };
}

MockEventSource.instances.length = 0;
const locks = new MockLockManager();
const hub = new MockBroadcastHub();
const BroadcastChannel = hub.channelClass();
const sharedStorage = createStorage();
const first = createTab({ locks, BroadcastChannel, localStorage: sharedStorage });
const second = createTab({ locks, BroadcastChannel, localStorage: sharedStorage });

assert(first.sandbox.MeiHostRuntimeEvents.isLeader(), "first visible tab must acquire leadership");
assert(!second.sandbox.MeiHostRuntimeEvents.isLeader(), "second tab must remain follower");
assert(MockEventSource.instances.length === 1, "two tabs must create only one Host EventSource");

MockEventSource.instances[0].emit("app-started", { appId: "mini-data" });
assert(
  second.received.some(
    (event) => event.type === "mei:host-event" && event.detail?.type === "app-started",
  ),
  "visible follower must receive relayed Host events",
);

first.setVisibility("hidden");
await wait(20);
assert(MockEventSource.instances[0].closed, "hidden leader must close its EventSource");
assert(second.sandbox.MeiHostRuntimeEvents.isLeader(), "visible follower must take leadership");
assert(MockEventSource.instances.length === 2, "takeover must open exactly one replacement stream");
assert(
  MockEventSource.instances.filter((sourceInstance) => !sourceInstance.closed).length === 1,
  "takeover must leave only one open stream",
);

const firstEventCount = first.received.length;
MockEventSource.instances[1].emit("app-stopped", { appId: "mini-data" });
assert(
  first.received.length === firstEventCount,
  "hidden follower must defer relayed events instead of updating in background",
);
first.setVisibility("visible");
assert(
  first.received.some(
    (event) => event.type === "mei:host-event" && event.detail?.type === "host-resync",
  ),
  "a dirty tab must request a full resync when visible again",
);

first.sandbox.MeiHostRuntimeEvents.disconnect("test-end");
second.sandbox.MeiHostRuntimeEvents.disconnect("test-end");

MockEventSource.instances.length = 0;
const hiddenFallback = createTab({
  locks: null,
  BroadcastChannel: undefined,
  localStorage: undefined,
  visibilityState: "hidden",
});
assert(
  MockEventSource.instances.length === 0,
  "an uncoordinated hidden tab must never open an EventSource",
);
hiddenFallback.sandbox.MeiHostRuntimeEvents.disconnect("test-end");

console.log("host-runtime-events-leader.test: ok");
