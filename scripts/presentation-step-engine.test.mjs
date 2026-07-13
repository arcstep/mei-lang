/**
 * Guard: presentation step engine manifest priority and deck-only fallback.
 * Run: node scripts/presentation-step-engine.test.mjs
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const enginePath = path.join(
  __dirname,
  "../app/assets/spa-navigation/presentation/presentation-step-engine.js",
);
const engineSource = await readFile(enginePath, "utf8");
const STORAGE_KEY = "mei_copilot_presentation_v1";

function manifest(id) {
  return {
    id,
    title: id,
    steps: [
      {
        id: `${id}-step`,
        caption: id,
        speaker_notes: `${id} notes`,
        actions: [{ type: "show_page", pageId: `${id}-page` }],
      },
    ],
  };
}

function createRuntime({ aot = null, dom = null, stored = null } = {}) {
  const nodes = new Map();
  const storage = new Map();
  if (stored) storage.set(STORAGE_KEY, JSON.stringify(stored));

  class HTMLElement {
    constructor(tagName = "div") {
      this.tagName = tagName.toUpperCase();
      this.id = "";
      this.textContent = "";
      this.parentElement = null;
      this.classList = {
        add() {},
        remove() {},
        contains() {
          return false;
        },
      };
    }
    setAttribute() {}
    removeAttribute() {}
    remove() {
      if (this.id) nodes.delete(this.id);
    }
  }
  class HTMLScriptElement extends HTMLElement {
    constructor() {
      super("script");
      this.type = "";
    }
  }
  const appendChild = (node) => {
    node.parentElement = document.head;
    if (node.id) nodes.set(node.id, node);
    return node;
  };
  const document = {
    getElementById(id) {
      return nodes.get(id) || null;
    },
    createElement(tagName) {
      return tagName === "script" ? new HTMLScriptElement() : new HTMLElement(tagName);
    },
    head: { appendChild },
    body: Object.assign(new HTMLElement("body"), { appendChild }),
    documentElement: { appendChild },
  };
  if (dom) {
    const node = new HTMLScriptElement();
    node.id = "mei-presentation-manifest";
    node.textContent = JSON.stringify(dom);
    nodes.set(node.id, node);
  }

  let nextDeckCalls = 0;
  const context = {
    console,
    document,
    HTMLElement,
    HTMLScriptElement,
    CustomEvent: class CustomEvent {},
    sessionStorage: {
      getItem(key) {
        return storage.get(key) || null;
      },
      setItem(key, value) {
        storage.set(key, String(value));
      },
      removeItem(key) {
        storage.delete(key);
      },
    },
    __mei: aot ? { presentation_map: { defaultScript: aot } } : {},
    __meiLangBoot: {
      nextDeckPage() {
        nextDeckCalls += 1;
        return true;
      },
      prevDeckPage() {
        return true;
      },
    },
  };
  context.window = context;
  context.global = context;
  context.globalThis = context;
  vm.createContext(context);
  vm.runInContext(engineSource, context);
  return {
    engine: context.__meiLangBoot.presentationStepEngine,
    storage,
    nextDeckCalls: () => nextDeckCalls,
  };
}

{
  const runtime = createRuntime({ aot: manifest("aot-default") });
  assert.equal(runtime.engine.ensureLoaded(), true);
  assert.equal(runtime.engine.state.manifest.id, "aot-default");
  assert.equal(runtime.engine.state.manifestSource, "aot");
  assert.equal(runtime.engine.currentStep().speaker_notes, "aot-default notes");
  assert.equal(runtime.storage.has(STORAGE_KEY), false, "AOT manifest must not become user session state");
}

{
  const runtime = createRuntime({
    aot: manifest("aot-default"),
    dom: manifest("advanced-dom"),
    stored: {
      __meiPresentationSession: true,
      source: "library",
      manifest: manifest("user-library"),
    },
  });
  assert.equal(runtime.engine.ensureLoaded(), true);
  assert.equal(runtime.engine.state.manifest.id, "user-library");
  assert.equal(runtime.engine.state.manifestSource, "library");
}

{
  const runtime = createRuntime({
    aot: manifest("aot-default"),
    dom: manifest("advanced-dom"),
  });
  assert.equal(runtime.engine.ensureLoaded(), true);
  assert.equal(runtime.engine.state.manifest.id, "advanced-dom");
  assert.equal(runtime.engine.state.manifestSource, "dom");
}

{
  const runtime = createRuntime({ aot: manifest("aot-default") });
  assert.equal(runtime.engine.ensureLoaded(), true);
  assert.equal(
    runtime.engine.replaceManifest(manifest("user-ephemeral"), { source: "ephemeral" }),
    true,
  );
  assert.equal(runtime.engine.state.manifest.id, "user-ephemeral");
  assert.equal(runtime.engine.state.manifestSource, "ephemeral");
}

{
  const runtime = createRuntime();
  assert.equal(runtime.engine.ensureLoaded(), false);
  assert.equal(runtime.engine.next(), true);
  assert.equal(runtime.nextDeckCalls(), 1, "no-manifest mode must keep direct deck flipping");
}

{
  const runtime = createRuntime({ aot: manifest("aot-default") });
  runtime.engine.clearSessionManifest();
  assert.equal(runtime.engine.ensureLoaded(), false, "explicit no-script choice suppresses AOT reload");
  assert.equal(runtime.engine.next(), true);
  assert.equal(runtime.nextDeckCalls(), 1);
}

console.log("presentation-step-engine.test.mjs: ok");
