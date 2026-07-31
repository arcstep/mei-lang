#!/usr/bin/env node

/**
 * Sync / check versions that remain inside the mei-lang public repo.
 * Viewer (tools/mei-viewer) and VSIX (tools/mei-lang-vscode) version
 * separately; align manually when cutting a coordinated release.
 */

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, "../..");
const mode = process.argv[2] ?? "--check";

if (!["--check", "--write", "--print-version"].includes(mode)) {
  console.error("usage: sync-versions.mjs [--check|--write|--print-version]");
  process.exit(2);
}

function readText(relativePath) {
  return readFileSync(resolve(root, relativePath), "utf8");
}

const cargoToml = readText("Cargo.toml");
const workspaceMatch = cargoToml.match(
  /\[workspace\.package\][\s\S]*?^\s*version\s*=\s*"([^"]+)"/m,
);
if (!workspaceMatch) {
  throw new Error("cannot read [workspace.package].version from Cargo.toml");
}
const version = workspaceMatch[1];

if (mode === "--print-version") {
  process.stdout.write(`${version}\n`);
  process.exit(0);
}

if (mode === "--write") {
  console.log(
    `mei-lang workspace version is ${version} (no in-repo Viewer/VSIX targets to sync)`,
  );
  process.exit(0);
}

console.log(`release versions are consistent: ${version}`);
process.exit(0);
