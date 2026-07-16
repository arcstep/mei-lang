#!/usr/bin/env node

import { createHash } from "node:crypto";
import {
  appendFileSync,
  mkdtempSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const root = resolve(scriptDir, "../..");
const fixture = mkdtempSync(resolve(tmpdir(), "mei-release-contract-"));
const version = "9.9.9";
const targets = [
  "aarch64-apple-darwin",
  "x86_64-apple-darwin",
  "x86_64-pc-windows-msvc",
  "x86_64-unknown-linux-gnu",
];

function sha256(path) {
  return createHash("sha256").update(Buffer.from(path)).digest("hex");
}

function writeAsset(product, target, extension) {
  const suffix = product === "viewer" && target.includes("windows")
    ? `${target}-setup.exe`
    : `${target}.${extension}`;
  const name = `mei-${product}-${version}-${suffix}`;
  const path = resolve(fixture, name);
  writeFileSync(path, `${product}:${target}\n`);
  const hash = createHash("sha256").update(Buffer.from(`${product}:${target}\n`)).digest("hex");
  const manifestName = name
    .replace(/\.tar\.gz$/, "")
    .replace(/\.(zip|exe)$/, "") + ".manifest.json";
  writeFileSync(
    resolve(fixture, manifestName),
    `${JSON.stringify({
      schemaVersion: 1,
      product,
      version,
      target,
      archive: name,
      bytes: statSync(path).size,
      sha256: hash,
      bins: [`mei-${product}`],
    }, null, 2)}\n`,
  );
  return name;
}

try {
  for (const target of targets.slice(0, 3)) {
    writeAsset("viewer", target, "zip");
  }
  for (const product of ["runtime", "toolchain"]) {
    for (const target of targets) {
      writeAsset(product, target, target.includes("windows") ? "zip" : "tar.gz");
    }
  }
  writeFileSync(resolve(fixture, `mei-lang-${version}.vsix`), "vsix\n");
  writeFileSync(resolve(fixture, `mei-lang-${version}.spdx.json`), "{}\n");

  const generate = spawnSync(
    process.execPath,
    [
      resolve(scriptDir, "generate-release-manifest.mjs"),
      "--assets-dir",
      fixture,
      "--version",
      version,
      "--git-sha",
      "a".repeat(40),
      "--tag",
      `v${version}`,
      "--channel",
      "stable",
    ],
    { cwd: root, stdio: "inherit" },
  );
  if (generate.status !== 0) process.exit(generate.status ?? 1);

  const verifyArgs = [
    resolve(scriptDir, "verify-release.mjs"),
    "--assets-dir",
    fixture,
  ];
  const verify = spawnSync(process.execPath, verifyArgs, { cwd: root, stdio: "inherit" });
  if (verify.status !== 0) process.exit(verify.status ?? 1);

  const tampered = resolve(
    fixture,
    `mei-toolchain-${version}-aarch64-apple-darwin.tar.gz`,
  );
  appendFileSync(tampered, "tampered");
  const rejected = spawnSync(process.execPath, verifyArgs, { cwd: root, stdio: "ignore" });
  if (rejected.status === 0) {
    throw new Error("release verifier accepted a tampered artifact");
  }

  console.log(`release manifest contract test passed (${sha256(fixture).slice(0, 8)})`);
} finally {
  rmSync(fixture, { recursive: true, force: true });
}
