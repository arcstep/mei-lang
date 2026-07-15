/**
 * Guard: empty / __no_client_bootstrap__ packs must NOT be treated as seedable Pack-First packs.
 * Run: node scripts/pack-first-empty-pack.test.mjs
 */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const runtimeQueryPath = path.join(
  __dirname,
  "../stock/components/dataset/runtime-query.js",
);

async function loadIsSeedableBootstrapPack() {
  const source = await readFile(runtimeQueryPath, "utf8");
  const match = source.match(
    /export function isSeedableBootstrapPack\([\s\S]*?\n\}\n/,
  );
  assert.ok(match, "isSeedableBootstrapPack export not found");
  const fnSource = match[0].replace(/^export /, "");
  const context = { console };
  vm.createContext(context);
  vm.runInContext(
    `const NO_CLIENT_BOOTSTRAP_REVISION = "__no_client_bootstrap__";\n${fnSource}\nthis.isSeedableBootstrapPack = isSeedableBootstrapPack;`,
    context,
  );
  return context.isSeedableBootstrapPack;
}

async function main() {
  const isSeedable = await loadIsSeedableBootstrapPack();

  assert.equal(
    isSeedable({
      metrics: [],
      payloadReady: true,
      clientRevision: "__no_client_bootstrap__",
    }),
    false,
    "empty no-client pack must not be seedable",
  );
  assert.equal(
    isSeedable({
      metrics: [],
      payloadReady: true,
      noClientPack: true,
    }),
    false,
    "noClientPack flag must disable Pack-First",
  );
  assert.equal(
    isSeedable({
      metrics: [],
      payloadReady: true,
      metaClientRevision: "__no_client_bootstrap__",
    }),
    false,
    "meta no-client revision must disable Pack-First",
  );
  assert.equal(
    isSeedable({
      metrics: [{ id: "devices_online" }],
      payloadReady: true,
      clientRevision: "abc123",
    }),
    true,
    "non-empty metrics must be seedable",
  );
  assert.equal(
    isSeedable({
      metrics: [],
      payloadReady: false,
      metaClientRevision: "abc123",
    }),
    true,
    "revision_only meta with real revision may wait for pack",
  );
  assert.equal(
    isSeedable({
      metrics: [],
      payloadReady: true,
      clientRevision: "abc123",
    }),
    false,
    "payloadReady with zero metrics must not wait 8s",
  );

  // Source-level guard: eval-pack-loader marks no-client packs.
  const loader = await readFile(
    path.join(__dirname, "../host-shell/app/assets/spa-navigation/spa/eval-pack-loader.js"),
    "utf8",
  );
  assert.match(loader, /__meiBootstrapNoClientPack/);
  assert.match(loader, /markNoClientBootstrapPack/);

  console.log("pack-first-empty-pack.test.mjs: ok");
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
