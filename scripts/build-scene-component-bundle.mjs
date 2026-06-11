#!/usr/bin/env node
/**
 * Build a single ESM bundle for one scene's workspace component entry scripts.
 *
 * Usage:
 *   node scripts/build-scene-component-bundle.mjs \
 *     --components-root /path/to/.stock/components \
 *     --entries chart/echarts/line.js,mei/text.js \
 *     --out /path/to/cache/home.abc123.js
 *
 *   node scripts/build-scene-component-bundle.mjs --revision-only ...  # prints revision to stdout
 */
import { createHash } from "node:crypto";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const __dirname = dirname(fileURLToPath(import.meta.url));

const WORKSPACE_EXTERNAL_PREFIX = "/workspace-components/";

function parseArgs(argv) {
  const out = { revisionOnly: false, entries: [], revision: "" };
  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--revision-only") {
      out.revisionOnly = true;
    } else if (arg === "--components-root") {
      out.componentsRoot = argv[++i];
    } else if (arg === "--entries") {
      out.entries = String(argv[++i] || "")
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean);
    } else if (arg === "--out") {
      out.out = argv[++i];
    } else if (arg === "--revision") {
      out.revision = String(argv[++i] || "").trim();
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  if (!out.componentsRoot) {
    throw new Error("--components-root is required");
  }
  if (!out.entries.length) {
    throw new Error("--entries is required");
  }
  if (!out.revisionOnly && !out.out) {
    throw new Error("--out is required unless --revision-only");
  }
  return out;
}

function workspaceExternalPlugin() {
  return {
    name: "mei-workspace-absolute-external",
    setup(buildApi) {
      buildApi.onResolve({ filter: /^\/workspace-components\// }, (args) => ({
        path: args.path,
        external: true,
      }));
    },
  };
}

function virtualEntrySource(entryRelPaths) {
  const imports = [...entryRelPaths]
    .sort()
    .map((rel) => `import ${JSON.stringify("./" + rel.replace(/\\/g, "/"))};`)
    .join("\n");
  return `// mei scene component bundle virtual entry\n${imports}\n`;
}

async function withVirtualEntryFile(componentsRoot, entryRelPaths, run) {
  const absRoot = resolve(componentsRoot);
  const entryPath = join(absRoot, "__mei_scene_bundle_entry__.mjs");
  await writeFile(entryPath, virtualEntrySource(entryRelPaths), "utf8");
  try {
    return await run(entryPath);
  } finally {
    await rm(entryPath, { force: true });
  }
}

async function analyzeBundleInputs(componentsRoot, entryRelPaths) {
  return withVirtualEntryFile(componentsRoot, entryRelPaths, async (entryPath) => {
    const result = await build({
      absWorkingDir: resolve(componentsRoot),
      entryPoints: [entryPath],
      bundle: true,
      write: false,
      metafile: true,
      format: "esm",
      platform: "browser",
      target: "es2020",
      logLevel: "silent",
      plugins: [workspaceExternalPlugin()],
    });
    return Object.keys(result.metafile.inputs)
      .filter((input) => !input.endsWith("__mei_scene_bundle_entry__.mjs"))
      .sort();
  });
}

async function computeRevision(componentsRoot, entryRelPaths) {
  const hash = createHash("sha256");
  const sortedEntries = [...entryRelPaths].map((item) => item.replace(/\\/g, "/")).sort();
  for (const rel of sortedEntries) {
    hash.update("entry:");
    hash.update(rel);
    hash.update("\0");
  }
  const absRoot = resolve(componentsRoot);
  const inputs = await analyzeBundleInputs(componentsRoot, sortedEntries);
  for (const inputPath of inputs) {
    const absPath = inputPath.startsWith("/") ? inputPath : join(absRoot, inputPath);
    const rel = relative(absRoot, absPath).replace(/\\/g, "/");
    const content = await readFile(absPath);
    hash.update("file:");
    hash.update(rel);
    hash.update("\0");
    hash.update(content);
  }
  return hash.digest("hex").slice(0, 16);
}

async function buildBundle(componentsRoot, entryRelPaths, outPath) {
  const absOut = resolve(outPath);
  await mkdir(dirname(absOut), { recursive: true });
  await withVirtualEntryFile(componentsRoot, entryRelPaths, async (entryPath) => {
    await build({
      absWorkingDir: resolve(componentsRoot),
      entryPoints: [entryPath],
      bundle: true,
      outfile: absOut,
      format: "esm",
      platform: "browser",
      target: "es2020",
      legalComments: "none",
      plugins: [workspaceExternalPlugin()],
    });
  });
  return absOut;
}

async function main() {
  const args = parseArgs(process.argv);
  const componentsRoot = resolve(args.componentsRoot);
  const entries = args.entries.map((item) => item.replace(/\\/g, "/"));
  const revision = args.revision || (await computeRevision(componentsRoot, entries));
  if (args.revisionOnly) {
    process.stdout.write(`${revision}\n`);
    return;
  }
  await buildBundle(componentsRoot, entries, args.out);
  process.stdout.write(`${revision}\n`);
  process.stderr.write(`[scene-bundle] wrote ${args.out} revision=${revision} entries=${entries.length}\n`);
}

main().catch((error) => {
  console.error("[scene-bundle] failed:", error?.message || error);
  process.exit(1);
});
