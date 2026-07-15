/**
 * Guard: presentation FAB/MDX deck controller reads AOT presentation_map.deck only.
 * Run: node scripts/presentation-focus-deck.test.mjs
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const focusControllerPath = path.join(
  __dirname,
  "../host-shell/app/assets/spa-navigation/presentation/focus-controller.js",
);

function extractFunction(source, name) {
  const start = source.indexOf(`function ${name}(`);
  assert.ok(start >= 0, `${name} not found`);
  let i = source.indexOf("{", start);
  assert.ok(i >= 0, `${name} body not found`);
  let depth = 0;
  for (; i < source.length; i += 1) {
    const ch = source[i];
    if (ch === "{") depth += 1;
    else if (ch === "}") {
      depth -= 1;
      if (depth === 0) {
        return source.slice(start, i + 1);
      }
    }
  }
  throw new Error(`unterminated ${name}`);
}

async function loadDeckHelpers() {
  const source = await readFile(focusControllerPath, "utf8");
  assert.ok(
    !source.includes("s-mission"),
    "focus-controller must not hardcode mini-data section page ids",
  );
  assert.ok(
    !source.includes("r-deck"),
    "focus-controller must not walk legacy r-deck/s-* section pages",
  );
  const readFn = extractFunction(source, "readPresentationDeck");
  const idsFn = extractFunction(source, "deckPageIds");
  const context = { globalThis: {}, window: undefined, CSS: { escape: (s) => s } };
  context.globalThis = context;
  vm.createContext(context);
  vm.runInContext(
    `${readFn}\n${idsFn}\nthis.readPresentationDeck = readPresentationDeck;\nthis.deckPageIds = deckPageIds;`,
    context,
  );
  return context;
}

async function main() {
  const helpers = await loadDeckHelpers();

  helpers.globalThis.__mei = {
    presentation_map: {
      deck: {
        stage_kind: "presentation",
        active_slide_id: "slide-02-why",
        slides: [
          { id: "slide-02-why", title: "Why", order: 1 },
          { id: "slide-01-cover", title: "Cover", order: 0, pattern: "full_bleed" },
          { id: "", title: "skip-me" },
        ],
      },
    },
  };

  const deck = helpers.readPresentationDeck();
  assert.equal(deck.stageKind, "presentation");
  assert.equal(deck.activeSlideId, "slide-02-why");
  assert.deepEqual(
    deck.slides.map((s) => s.id),
    ["slide-01-cover", "slide-02-why"],
  );
  assert.equal(deck.slides[0].pattern, "full_bleed");
  assert.equal(helpers.deckPageIds().join(","), "slide-01-cover,slide-02-why");

  helpers.globalThis.__mei = { presentation_map: {} };
  assert.equal(helpers.readPresentationDeck(), null);
  assert.equal(helpers.deckPageIds().length, 0);

  console.log("presentation-focus-deck.test.mjs: ok");
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
