#!/usr/bin/env node
/**
 * Package mei-viewer build outputs into versioned files under desktop/dist/.
 *
 * macOS:
 *   dist/mei-viewer.app                          # stable, open without unzip (gitignored)
 *   dist/mei-viewer-<ver>-<target-triple>.zip
 * Windows:
 *   dist/mei-viewer-<ver>-<target-triple>-setup.exe
 *
 * Version = tauri.conf.json#version. Git SHA is recorded in the side manifest.
 *
 * Does NOT delete the Tauri-built .app under src-tauri/target/.../bundle/macos/.
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
import { createHash } from "node:crypto";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { platform, arch } from "node:os";

const __dirname = dirname(fileURLToPath(import.meta.url));
const desktopRoot = resolve(__dirname, "..");
const meiLangRoot = resolve(desktopRoot, "..");
const confPath = join(desktopRoot, "src-tauri", "tauri.conf.json");
const distRoot = join(desktopRoot, "dist");

/** Prefer the newest Tauri release bundle (src-tauri/target or CARGO_TARGET_DIR). */
function resolveReleaseTargetRoot() {
  const candidates = [];
  if (process.env.CARGO_TARGET_DIR) {
    candidates.push(resolve(process.env.CARGO_TARGET_DIR, "release"));
  }
  candidates.push(join(desktopRoot, "src-tauri", "target", "release"));
  candidates.push(join(meiLangRoot, "target", "release"));
  const existing = [...new Set(candidates)].filter((root) =>
    existsSync(join(root, "bundle")),
  );
  if (!existing.length) {
    return join(desktopRoot, "src-tauri", "target", "release");
  }
  existing.sort((a, b) => {
    const appA = join(a, "bundle", "macos", "mei-viewer.app");
    const appB = join(b, "bundle", "macos", "mei-viewer.app");
    const mtime = (p) => {
      try {
        return spawnSync("stat", ["-f", "%m", p], { encoding: "utf8" }).stdout.trim() || "0";
      } catch {
        return "0";
      }
    };
    return Number(mtime(appB)) - Number(mtime(appA));
  });
  return existing[0];
}
function readVersion() {
  const conf = JSON.parse(readFileSync(confPath, "utf8"));
  return String(conf.version || "0.0.0").trim();
}

function readGitSha() {
  const git = spawnSync("git", ["rev-parse", "--short", "HEAD"], {
    cwd: resolve(desktopRoot, ".."),
    encoding: "utf8",
  });
  return git.status === 0 ? git.stdout.trim() || null : null;
}

function targetTriple() {
  if (platform() === "darwin") {
    return arch() === "arm64" ? "aarch64-apple-darwin" : "x86_64-apple-darwin";
  }
  if (platform() === "win32" && arch() === "x64") {
    return "x86_64-pc-windows-msvc";
  }
  throw new Error(`unsupported release target ${platform()}-${arch()}`);
}

function run(cmd, args) {
  const r = spawnSync(cmd, args, { stdio: "inherit" });
  if (r.status !== 0) {
    throw new Error(`${cmd} failed with status ${r.status}`);
  }
}

function zipMacApp(appDir, outZip) {
  if (platform() !== "darwin") {
    throw new Error("macOS .app packaging requires running on darwin");
  }
  // Tauri/linker adhoc signatures often break across zip round-trips
  // ("code has no resources but signature indicates they must be present").
  // Re-sign deep before archive so extract+open works for local/CI zips.
  run("codesign", ["--force", "--deep", "--sign", "-", appDir]);
  // Prefer plain zip (no __MACOSX AppleDouble). Finder/ditto extract keep structure.
  run("ditto", ["-c", "-k", "--keepParent", appDir, outZip]);
}

/** Copy .app into dist/mei-viewer.app (stable path for Finder / open). */
function syncStableMacApp(appDir, destApp) {
  rmSync(destApp, { recursive: true, force: true });
  run("ditto", [appDir, destApp]);
  run("codesign", ["--force", "--deep", "--sign", "-", destApp]);
}

function cleanDistArtifacts() {
  if (!existsSync(distRoot)) return;
  for (const name of readdirSync(distRoot)) {
    // Keep dist/mei-viewer.app across rebuilds until we replace it.
    if (name === "mei-viewer.app") continue;
    if (name.startsWith("mei-viewer-") || name.endsWith(".manifest.json")) {
      rmSync(join(distRoot, name), { recursive: true, force: true });
    }
  }
}

function main() {
  const version = readVersion();
  const gitSha = readGitSha();
  const target = targetTriple();
  const targetRoot = resolveReleaseTargetRoot();
  mkdirSync(distRoot, { recursive: true });
  cleanDistArtifacts();

  const wrote = [];
  let artifact = null;
  let openHint = null;

  if (platform() === "darwin") {
    const appPath = join(targetRoot, "bundle", "macos", "mei-viewer.app");
    if (!existsSync(appPath)) {
      throw new Error(`missing ${appPath}; run npm run build first`);
    }
    console.log(`[package-release] source app: ${appPath}`);
    const stableApp = join(distRoot, "mei-viewer.app");
    syncStableMacApp(appPath, stableApp);
    wrote.push(stableApp);
    openHint = stableApp;

    const zipName = `mei-viewer-${version}-${target}.zip`;
    const outZip = join(distRoot, zipName);
    zipMacApp(appPath, outZip);
    wrote.push(outZip);
    artifact = zipName;
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
    const outputName = `mei-viewer-${version}-${target}-setup.exe`;
    const outPath = join(distRoot, outputName);
    copyFileSync(src, outPath);
    wrote.push(outPath);
    artifact = outputName;
    openHint = outPath;
  } else {
    console.warn(`[package-release] unsupported platform ${platform()}; nothing packaged`);
  }

  const manifest = {
    schemaVersion: 1,
    product: "viewer",
    version,
    gitSha,
    target,
    platform: platform(),
    arch: arch(),
    artifact,
    openPath: openHint ? openHint.split(/[/\\]/).pop() : null,
    includedComponents: [
      "mei-host-shell",
      "mei-app-runtime",
      "mei-plug-ds",
      "mei-snapshot",
      "mei-compiler",
      "app-assets",
      "stock-components",
      "stock-templates",
    ],
  };
  if (!artifact) {
    throw new Error(`no viewer artifact produced for ${platform()}-${arch()}`);
  }
  const artifactPath = join(distRoot, artifact);
  manifest.bytes = readFileSync(artifactPath).byteLength;
  manifest.sha256 = createHash("sha256")
    .update(readFileSync(artifactPath))
    .digest("hex");
  const manifestBase = artifact.replace(/\.(zip|exe)$/, "");
  const manifestPath = join(distRoot, `${manifestBase}.manifest.json`);
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
  wrote.push(manifestPath);
  const summaryPath = join(distRoot, "MANIFEST.json");
  writeFileSync(summaryPath, JSON.stringify(manifest, null, 2) + "\n");
  wrote.push(summaryPath);

  console.log("[package-release] wrote:");
  for (const p of wrote) console.log(`  ${p}`);
  if (openHint) {
    console.log(`[package-release] open without unzip:`);
    console.log(`  open "${openHint}"`);
  }
}

main();
