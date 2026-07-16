#!/usr/bin/env node

import { readFileSync, writeFileSync } from "node:fs";
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

function writeText(relativePath, content) {
  writeFileSync(resolve(root, relativePath), content);
}

function readJson(relativePath) {
  return JSON.parse(readText(relativePath));
}

function writeJson(relativePath, value) {
  writeText(relativePath, `${JSON.stringify(value, null, 2)}\n`);
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

const jsonTargets = [
  ["desktop/package.json", (doc) => doc.version],
  ["desktop/src-tauri/tauri.conf.json", (doc) => doc.version],
  ["desktop/package-lock.json", (doc) => doc.version],
  ["extensions/mei-lang-vscode/package.json", (doc) => doc.version],
  ["extensions/mei-lang-vscode/package-lock.json", (doc) => doc.version],
];

const mismatches = [];
for (const [relativePath, getVersion] of jsonTargets) {
  const document = readJson(relativePath);
  if (getVersion(document) !== version) {
    mismatches.push(`${relativePath}: ${getVersion(document) ?? "<missing>"} != ${version}`);
  }
  if (relativePath.endsWith("package-lock.json")) {
    const rootPackageVersion = document.packages?.[""]?.version;
    if (rootPackageVersion !== version) {
      mismatches.push(
        `${relativePath}#packages[""].version: ${rootPackageVersion ?? "<missing>"} != ${version}`,
      );
    }
  }
}

const desktopCargoPath = "desktop/src-tauri/Cargo.toml";
const desktopCargo = readText(desktopCargoPath);
const desktopCargoMatch = desktopCargo.match(
  /\[package\][\s\S]*?^\s*version\s*=\s*"([^"]+)"/m,
);
if (!desktopCargoMatch) {
  throw new Error(`cannot read [package].version from ${desktopCargoPath}`);
}
if (desktopCargoMatch[1] !== version) {
  mismatches.push(`${desktopCargoPath}: ${desktopCargoMatch[1]} != ${version}`);
}

const desktopLockPath = "desktop/src-tauri/Cargo.lock";
const desktopLock = readText(desktopLockPath);
for (const packageName of ["mei-desktop-viewer", "mei-snapshot"]) {
  const pattern = new RegExp(
    `(\\[\\[package\\]\\]\\nname = "${packageName}"\\nversion = ")([^"]+)(")`,
  );
  const match = desktopLock.match(pattern);
  if (!match) {
    throw new Error(`cannot read ${packageName} version from ${desktopLockPath}`);
  }
  if (match[2] !== version) {
    mismatches.push(`${desktopLockPath}#${packageName}: ${match[2]} != ${version}`);
  }
}

if (mode === "--check") {
  if (mismatches.length > 0) {
    console.error(`release version mismatch (Cargo.toml = ${version}):`);
    for (const mismatch of mismatches) console.error(`  - ${mismatch}`);
    console.error("run: node scripts/release/sync-versions.mjs --write");
    process.exit(1);
  }
  console.log(`release versions are consistent: ${version}`);
  process.exit(0);
}

for (const [relativePath] of jsonTargets) {
  const document = readJson(relativePath);
  document.version = version;
  if (relativePath.endsWith("package-lock.json") && document.packages?.[""]) {
    document.packages[""].version = version;
  }
  writeJson(relativePath, document);
}

writeText(
  desktopCargoPath,
  desktopCargo.replace(
    /(\[package\][\s\S]*?^\s*version\s*=\s*")[^"]+(")/m,
    `$1${version}$2`,
  ),
);

let updatedDesktopLock = desktopLock;
for (const packageName of ["mei-desktop-viewer", "mei-snapshot"]) {
  const pattern = new RegExp(
    `(\\[\\[package\\]\\]\\nname = "${packageName}"\\nversion = ")[^"]+(")`,
  );
  updatedDesktopLock = updatedDesktopLock.replace(pattern, `$1${version}$2`);
}
writeText(desktopLockPath, updatedDesktopLock);

console.log(`synchronized release versions to ${version}`);
