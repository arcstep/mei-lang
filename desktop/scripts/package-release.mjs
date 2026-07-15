#!/usr/bin/env node
/**
 * Package mei-viewer build outputs into versioned files under desktop/dist/.
 *
 * macOS:   dist/mei-viewer-<ver>-<arch>-apple-darwin.zip  (contains mei-viewer.app)
 * Windows: dist/mei-viewer-<ver>-x64-setup.exe
 *
 * Version = tauri.conf.json#version + optional +gitShortSha
 */
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { platform, arch } from "node:os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const desktopRoot = resolve(__dirname, "..");
const confPath = join(desktopRoot, "src-tauri", "tauri.conf.json");
const targetRoot = join(desktopRoot, "src-tauri", "target", "release");
const distRoot = join(desktopRoot, "dist");

function readVersion() {
  const conf = JSON.parse(readFileSync(confPath, "utf8"));
  let ver = String(conf.version || "0.0.0").trim();
  const git = spawnSync("git", ["rev-parse", "--short", "HEAD"], {
    cwd: resolve(desktopRoot, ".."),
    encoding: "utf8",
  });
  if (git.status === 0) {
    const sha = git.stdout.trim();
    if (sha) ver = `${ver}+${sha}`;
  }
  return ver.replace(/[^A-Za-z0-9._+-]/g, "-");
}

function darwinArchLabel() {
  return arch() === "arm64" ? "aarch64" : "x86_64";
}

function zipMacApp(appDir, outZip) {
  if (platform() !== "darwin") {
    throw new Error("macOS .app packaging requires running on darwin");
  }
  const r = spawnSync(
    "ditto",
    ["-c", "-k", "--sequesterRsrc", "--keepParent", appDir, outZip],
    { stdio: "inherit" }
  );
  if (r.status !== 0) {
    throw new Error(`ditto failed with status ${r.status}`);
  }
}

function main() {
  const version = readVersion();
  mkdirSync(distRoot, { recursive: true });
  for (const name of readdirSync(distRoot)) {
    if (name.startsWith("mei-viewer-") || name === "MANIFEST.json") {
      rmSync(join(distRoot, name), { recursive: true, force: true });
    }
  }

  const wrote = [];

  if (platform() === "darwin") {
    const appPath = join(targetRoot, "bundle", "macos", "mei-viewer.app");
    if (!existsSync(appPath)) {
      throw new Error(`missing ${appPath}; run npm run build first`);
    }
    const zipName = `mei-viewer-${version}-${darwinArchLabel()}-apple-darwin.zip`;
    const outZip = join(distRoot, zipName);
    zipMacApp(appPath, outZip);
    wrote.push(outZip);
  } else if (platform() === "win32") {
    const nsisDir = join(targetRoot, "bundle", "nsis");
    if (!existsSync(nsisDir)) {
      throw new Error(`missing ${nsisDir}; run npm run build first`);
    }
    const exes = readdirSync(nsisDir).filter((f) => f.endsWith(".exe"));
    if (!exes.length) {
      throw new Error(`no NSIS .exe under ${nsisDir}`);
    }
    const src = join(nsisDir, exes.find((f) => /setup/i.test(f)) || exes[0]);
    const outPath = join(distRoot, `mei-viewer-${version}-x64-setup.exe`);
    copyFileSync(src, outPath);
    wrote.push(outPath);
  } else {
    console.warn(`[package-release] unsupported platform ${platform()}; nothing packaged`);
  }

  const manifest = {
    format: "mei-viewer-dist",
    formatVersion: 1,
    version,
    productName: "mei-viewer",
    platform: platform(),
    arch: arch(),
    files: wrote.map((p) => p.split(/[/\\]/).pop()),
  };
  const manifestPath = join(distRoot, "MANIFEST.json");
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
  wrote.push(manifestPath);

  console.log("[package-release] wrote:");
  for (const p of wrote) console.log(`  ${p}`);
}

main();
