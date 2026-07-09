/**
 * One-time splitter: spa-navigation.js -> spa-navigation/ directory modules
 * Preserves original line order inside a single IIFE (preamble + modules + epilogue).
 */
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const assetsRoot = path.join(root, "app", "assets");
const srcPath = path.join(assetsRoot, "spa-navigation.js");
const outDir = path.join(assetsRoot, "spa-navigation");

/** @type {{ rel: string, start: number, end: number }[]} */
const MODULES = [
  { rel: "preamble.js", start: 1, end: 5 },
  { rel: "constants.js", start: 6, end: 42 },
  { rel: "events.js", start: 44, end: 67 },
  { rel: "constants-maps.js", start: 68, end: 91 },
  { rel: "drilldown/column-infer.js", start: 93, end: 136 },
  { rel: "nav-state.js", start: 137, end: 141 },
  { rel: "route-predicates.js", start: 143, end: 207 },
  { rel: "drilldown/scene-shell-normalize.js", start: 209, end: 453 },
  { rel: "drilldown/t2-page-link.js", start: 455, end: 543 },
  { rel: "primitives.js", start: 545, end: 619 },
  { rel: "drilldown/tab-model-runtime.js", start: 621, end: 949 },
  { rel: "drilldown/tab-model-overrides.js", start: 950, end: 1133 },
  { rel: "drilldown/tab-model-config.js", start: 1135, end: 1498 },
  { rel: "drilldown/config-slots.js", start: 1500, end: 1838 },
  { rel: "drilldown/config-legacy.js", start: 1840, end: 2134 },
  { rel: "drilldown/config-open.js", start: 2136, end: 2204 },
  { rel: "drilldown/debug.js", start: 2206, end: 2265 },
  { rel: "drilldown/data-fetch.js", start: 2267, end: 2453 },
  { rel: "drilldown/row-aggregation.js", start: 2455, end: 2560 },
  { rel: "drilldown/props-builders.js", start: 2562, end: 2737 },
  { rel: "drilldown/widget-mount.js", start: 2739, end: 2996 },
  { rel: "drilldown/render-analytics.js", start: 2998, end: 3155 },
  { rel: "drilldown/render-list-preview.js", start: 3157, end: 3315 },
  { rel: "drilldown/render-structured.js", start: 3317, end: 3547 },
  { rel: "drilldown/render-derived.js", start: 3549, end: 3657 },
  { rel: "drilldown/render-generic.js", start: 3659, end: 3870 },
  { rel: "drilldown/overlay-chrome.js", start: 3872, end: 3947 },
  { rel: "drilldown/storage.js", start: 3949, end: 4046 },
  { rel: "drilldown/projection-host.js", start: 4048, end: 4151 },
  { rel: "drilldown/context-banner.js", start: 4153, end: 4241 },
  { rel: "spa/loading-ui.js", start: 4243, end: 4397 },
  { rel: "spa/url-policy.js", start: 4399, end: 4536 },
  { rel: "spa/script-sync.js", start: 4538, end: 4677 },
  { rel: "spa/script-loader.js", start: 4679, end: 4733 },
  { rel: "spa/manage-preview.js", start: 4735, end: 4758 },
  { rel: "spa/dom-swap.js", start: 4760, end: 5127 },
  { rel: "spa/post-navigation.js", start: 5129, end: 5169 },
  { rel: "spa/navigation.js", start: 5171, end: 5302 },
  { rel: "epilogue.js", start: 5304, end: 5342 },
];

const src = await readFile(srcPath, "utf8");
const lines = src.split("\n");
const lastLine = lines.length;

// Extend each module through blank gaps so concatenation reproduces the original file.
const sorted = [...MODULES].sort((a, b) => a.start - b.start);
for (let i = 0; i < sorted.length; i++) {
  const nextStart = i < sorted.length - 1 ? sorted[i + 1].start : lastLine + 1;
  sorted[i].end = nextStart - 1;
}

for (const mod of MODULES) {
  const chunk = lines.slice(mod.start - 1, mod.end).join("\n");
  const outPath = path.join(outDir, mod.rel);
  await mkdir(path.dirname(outPath), { recursive: true });
  await writeFile(outPath, chunk + "\n", "utf8");
  const lineCount = mod.end - mod.start + 1;
  if (lineCount > 500) {
    console.warn(`WARN ${mod.rel}: ${lineCount} lines (>500)`);
  } else {
    console.log(`OK ${mod.rel}: ${lineCount} lines`);
  }
}

// Verify full coverage of body lines 1..lastLine
const covered = new Set();
for (const mod of MODULES) {
  for (let i = mod.start; i <= mod.end; i++) covered.add(i);
}
const missing = [];
for (let i = 1; i <= lastLine; i++) {
  if (!covered.has(i)) missing.push(i);
}
if (missing.length) {
  throw new Error(`Uncovered lines: ${missing.slice(0, 20).join(", ")}${missing.length > 20 ? "..." : ""}`);
}

console.log(`Split complete: ${MODULES.length} modules`);
