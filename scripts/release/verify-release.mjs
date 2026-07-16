#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, "../..");
const assetsFlag = process.argv.indexOf("--assets-dir");
const assetsDir = resolve(
  root,
  assetsFlag >= 0 ? process.argv[assetsFlag + 1] : "dist/release",
);

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

const manifest = JSON.parse(
  readFileSync(resolve(assetsDir, "release-manifest.json"), "utf8"),
);
if (manifest.schemaVersion !== 1) throw new Error("unsupported release manifest schema");
if (!manifest.version || !manifest.gitSha) {
  throw new Error("release manifest requires version and gitSha");
}

const expectedTargets = {
  viewer: new Set([
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
  ]),
  toolchain: new Set([
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
  ]),
  runtime: new Set([
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
  ]),
};

for (const [product, expected] of Object.entries(expectedTargets)) {
  const actual = new Set(
    manifest.assets
      .filter((asset) => asset.product === product)
      .map((asset) => asset.target),
  );
  const missing = [...expected].filter((target) => !actual.has(target));
  const unexpected = [...actual].filter((target) => !expected.has(target));
  if (missing.length || unexpected.length) {
    throw new Error(
      `${product} target mismatch; missing=${missing.join(",") || "-"} unexpected=${unexpected.join(",") || "-"}`,
    );
  }
}

for (const product of ["vscode-extension", "sbom"]) {
  if (manifest.assets.filter((asset) => asset.product === product).length !== 1) {
    throw new Error(`release requires exactly one ${product} asset`);
  }
}

for (const asset of manifest.assets) {
  const path = resolve(assetsDir, asset.file);
  if (!statSync(path).isFile()) throw new Error(`missing release asset: ${asset.file}`);
  if (statSync(path).size !== asset.bytes) {
    throw new Error(`${asset.file}: byte size does not match release manifest`);
  }
  if (sha256(path) !== asset.sha256) {
    throw new Error(`${asset.file}: SHA-256 does not match release manifest`);
  }
  if (asset.metadataFile && !statSync(resolve(assetsDir, asset.metadataFile)).isFile()) {
    throw new Error(`${asset.file}: missing metadata file ${asset.metadataFile}`);
  }
}

const checksumLines = readFileSync(resolve(assetsDir, "SHA256SUMS.txt"), "utf8")
  .trim()
  .split("\n");
const checksumFiles = new Set();
for (const line of checksumLines) {
  const match = line.match(/^([a-f0-9]{64})  (.+)$/);
  if (!match) throw new Error(`invalid SHA256SUMS line: ${line}`);
  const [, expected, name] = match;
  checksumFiles.add(name);
  if (sha256(resolve(assetsDir, name)) !== expected) {
    throw new Error(`${name}: SHA-256 does not match SHA256SUMS.txt`);
  }
}

const filesRequiringChecksum = readdirSync(assetsDir)
  .filter((name) => name !== "SHA256SUMS.txt")
  .filter((name) => statSync(resolve(assetsDir, name)).isFile());
for (const name of filesRequiringChecksum) {
  if (!checksumFiles.has(name)) throw new Error(`SHA256SUMS.txt omits ${name}`);
}

console.log(
  `verified release ${manifest.version}: ${manifest.assets.length} assets, ${checksumFiles.size} checksums`,
);
