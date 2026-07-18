/**
 * Guard: AOT deck script is exposed as a read-only client library entry.
 * Run: node scripts/presentation-script-library.test.mjs
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const libraryPath = path.join(
  __dirname,
  "../../host-shell/app/assets/spa-navigation/presentation/presentation-script-library.js",
);
const source = await readFile(libraryPath, "utf8");
const defaultScript = {
  id: "compiled-deck",
  title: "编译期讲稿",
  steps: [{ id: "cover", actions: [{ type: "show_page", pageId: "slide-cover" }] }],
};
let compileCalls = 0;

const context = {
  console,
  location: { pathname: "/apps/demo/intro" },
  __mei: { presentation_map: { defaultScript } },
  __meiLangBoot: {
    copilotFabContext: {
      resolveStageTargetKey() {
        return "presentation/intro";
      },
    },
    compileEphemeralPresentation() {
      compileCalls += 1;
      throw new Error("AOT script must not be recompiled");
    },
  },
  fetch: async () => ({
    ok: true,
    async json() {
      return {
        appId: "demo",
        defaultScriptId: "",
        defaultByStage: {},
        scripts: [],
        imageAssets: {},
      };
    },
  }),
};
context.window = context;
context.globalThis = context;
vm.createContext(context);
vm.runInContext(source, context);

const library = context.__meiLangBoot.presentationScriptLibrary;
const listed = await library.listScripts("demo");
assert.equal(listed.scripts.length, 1);
assert.equal(listed.scripts[0].id, "deck-default");
assert.equal(listed.scripts[0].title, "编译期讲稿");
assert.equal(listed.scripts[0].sourceKind, "aot");
assert.equal(listed.scripts[0].readOnly, true);
assert.equal(listed.scripts[0].target, "presentation/intro");
assert.equal(listed.defaultScriptId, "deck-default");

const loaded = await library.loadAndCompileScript("deck-default", { appId: "demo" });
assert.deepEqual(loaded.result.manifest, defaultScript);
assert.equal(loaded.result.sourceKind, "aot");
assert.equal(compileCalls, 0);

await assert.rejects(
  library.saveScript("deck-default", "anything", { appId: "demo" }),
  /不能保存/,
);

console.log("presentation-script-library.test.mjs: ok");
