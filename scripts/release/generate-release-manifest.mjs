#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, "../..");
const options = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  options.set(process.argv[index], process.argv[index + 1]);
}

const assetsDir = resolve(root, options.get("--assets-dir") ?? "dist/release");
const version =
  options.get("--version") ??
  readFileSync(resolve(root, "Cargo.toml"), "utf8").match(
    /\[workspace\.package\][\s\S]*?^\s*version\s*=\s*"([^"]+)"/m,
  )?.[1];
const gitSha = options.get("--git-sha") ?? process.env.GITHUB_SHA ?? null;
const tag = options.get("--tag") || null;
const channel = options.get("--channel") ?? "dry-run";
const repository = process.env.GITHUB_REPOSITORY ?? "arcstep/mei-lang";

if (!version) throw new Error("release version is required");

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function targetInfo(target) {
  if (!target) return { os: "any", arch: "any" };
  if (target.includes("windows")) return { os: "windows", arch: "x86_64" };
  if (target.includes("apple-darwin")) {
    return { os: "macos", arch: target.startsWith("aarch64") ? "aarch64" : "x86_64" };
  }
  if (target.includes("linux")) {
    return { os: "linux", arch: target.startsWith("aarch64") ? "aarch64" : "x86_64" };
  }
  throw new Error(`unsupported target in release metadata: ${target}`);
}

function readSideManifest(artifactName) {
  const manifestName = artifactName
    .replace(/\.tar\.gz$/, "")
    .replace(/\.(zip|exe)$/, "") + ".manifest.json";
  const manifestPath = resolve(assetsDir, manifestName);
  try {
    return { name: manifestName, document: JSON.parse(readFileSync(manifestPath, "utf8")) };
  } catch {
    return null;
  }
}

function classify(name) {
  if (name.startsWith(`mei-viewer-${version}-`)) return "viewer";
  if (name.startsWith(`mei-toolchain-${version}-`)) return "toolchain";
  if (name.startsWith(`mei-runtime-${version}-`)) return "runtime";
  if (name === `mei-lang-${version}.vsix`) return "vscode-extension";
  if (name === `mei-lang-${version}.spdx.json`) return "sbom";
  return null;
}

const ignored = new Set(["release-manifest.json", "SHA256SUMS.txt"]);
const names = readdirSync(assetsDir)
  .filter((name) => statSync(resolve(assetsDir, name)).isFile())
  .filter((name) => !ignored.has(name))
  .sort();

const sideManifestNames = new Set(names.filter((name) => name.endsWith(".manifest.json")));
const assets = [];
for (const name of names) {
  if (sideManifestNames.has(name)) continue;
  const product = classify(name);
  if (!product) {
    throw new Error(`unrecognized release asset: ${name}`);
  }
  const path = resolve(assetsDir, name);
  const side = readSideManifest(name);
  const target = side?.document.target ?? null;
  const { os, arch } = targetInfo(target);
  const expectedHash = side?.document.sha256;
  const actualHash = sha256(path);
  if (expectedHash && expectedHash !== actualHash) {
    throw new Error(`${name}: side manifest SHA-256 does not match artifact`);
  }
  assets.push({
    product,
    target,
    os,
    arch,
    file: name,
    bytes: statSync(path).size,
    sha256: actualHash,
    mediaType:
      extname(name) === ".vsix"
        ? "application/vsix"
        : name.endsWith(".tar.gz")
          ? "application/gzip"
          : name.endsWith(".zip")
            ? "application/zip"
            : name.endsWith(".exe")
              ? "application/vnd.microsoft.portable-executable"
              : "application/spdx+json",
    components: side?.document.bins ?? side?.document.includedComponents ?? [],
    metadataFile: side?.name ?? null,
    githubUrl: tag
      ? `https://github.com/${repository}/releases/download/${tag}/${name}`
      : null,
    mirrorUrls: [],
  });
}

for (const manifestName of sideManifestNames) {
  const manifest = JSON.parse(readFileSync(resolve(assetsDir, manifestName), "utf8"));
  if (manifest.version !== version) {
    throw new Error(`${manifestName}: version ${manifest.version} does not match ${version}`);
  }
}

const requiredProducts = ["viewer", "toolchain", "runtime", "vscode-extension", "sbom"];
for (const product of requiredProducts) {
  if (!assets.some((asset) => asset.product === product)) {
    throw new Error(`release is missing required product: ${product}`);
  }
}

const releaseManifest = {
  schemaVersion: 1,
  version,
  channel,
  tag,
  gitSha,
  publishedAt: new Date().toISOString(),
  signing: {
    status: "unsigned",
    note: "Platform signing is not configured; SHA-256 and GitHub attestations remain mandatory.",
  },
  provenance: {
    provider: "github-artifact-attestations",
  },
  assets,
  metadataFiles: [...sideManifestNames].sort(),
};

writeFileSync(
  resolve(assetsDir, "release-manifest.json"),
  `${JSON.stringify(releaseManifest, null, 2)}\n`,
);

const checksumNames = readdirSync(assetsDir)
  .filter((name) => name !== "SHA256SUMS.txt")
  .filter((name) => statSync(resolve(assetsDir, name)).isFile())
  .sort();
const checksums = checksumNames
  .map((name) => `${sha256(resolve(assetsDir, name))}  ${basename(name)}`)
  .join("\n");
writeFileSync(resolve(assetsDir, "SHA256SUMS.txt"), `${checksums}\n`);

console.log(`generated release-manifest.json with ${assets.length} assets`);
