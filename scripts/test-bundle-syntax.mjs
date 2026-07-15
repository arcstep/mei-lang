import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { transform } from "esbuild";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const distRoot = path.join(root, "host-shell", "app", "assets", "dist");

const bundles = [
  "manage.bundle.js",
  "access.bundle.js",
  "config.bundle.js",
  "upload.bundle.js",
  "manage-source.bundle.js",
  "shoelace.bundle.js",
  "auth-rsa.bundle.js",
];

async function assertBundleSyntax(name) {
  const filePath = path.join(distRoot, name);
  let source;
  try {
    source = await readFile(filePath, "utf8");
  } catch {
    return;
  }
  await transform(source, {
    loader: "js",
    target: "es2020",
    sourcefile: name,
  });
  if (name === "manage.bundle.js" && !source.includes("function measureStageContentSize(")) {
    throw new Error(`${name} missing measureStageContentSize`);
  }
  if (name === "manage.bundle.js" && /function measureStageContentSize\(\)\s*;\s*\{/.test(source)) {
    throw new Error(`${name} has broken measureStageContentSize body (split fragment semicolon)`);
  }
}

for (const bundle of bundles) {
  await assertBundleSyntax(bundle);
}

console.log("[assets:build] bundle syntax ok");
