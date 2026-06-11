/**
 * Smoke test for scene component bundle builder (stock components).
 */
import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const componentsRoot = join(root, "stock", "components");
const scriptPath = join(root, "scripts", "build-scene-component-bundle.mjs");
const entries = ["mei/text.js", "cockpit/drilldown-meta.js"];
const outPath = join(root, "target", "test-scene-bundle", "home.test.js");

async function runNode(args) {
  const { stdout, stderr } = await execFileAsync(process.execPath, [scriptPath, ...args], {
    cwd: root,
    maxBuffer: 10 * 1024 * 1024,
  });
  return { stdout: String(stdout || ""), stderr: String(stderr || "") };
}

const { stdout: revisionStdout } = await runNode([
  "--revision-only",
  "--components-root",
  componentsRoot,
  "--entries",
  entries.join(","),
]);
const revision = revisionStdout.trim();
assert.match(revision, /^[a-f0-9]{16}$/, "revision must be 16 hex chars");

await runNode([
  "--components-root",
  componentsRoot,
  "--entries",
  entries.join(","),
  "--out",
  outPath,
]);
await access(outPath);
const bundle = await readFile(outPath, "utf8");
assert.ok(bundle.includes("customElements"), "bundle should register custom elements");
assert.ok(bundle.includes("mei-text"), "bundle should include text component tag");

console.log("test-scene-bundle ok", { revision, outPath });
