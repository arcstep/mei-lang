#!/usr/bin/env node
/**
 * 兼容/调试用 presentation 编译 CLI。
 * 正式架构中，MDX 通过服务端临时 compile API lower 为 session manifest。
 */
import fs from "node:fs";
import path from "node:path";
import { compileMdxToManifest } from "../lib/presentation-compile-core.mjs";

function collectPresentationFiles(dirPath) {
  const files = [];
  for (const entry of fs.readdirSync(dirPath, { withFileTypes: true })) {
    const fullPath = path.join(dirPath, entry.name);
    if (entry.isDirectory()) {
      files.push(...collectPresentationFiles(fullPath));
      continue;
    }
    if (/\.presentation\.mdx$/i.test(entry.name)) {
      files.push(fullPath);
    }
  }
  return files.sort();
}

function readStdinUtf8() {
  return new Promise((resolve, reject) => {
    const chunks = [];
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => chunks.push(chunk));
    process.stdin.on("end", () => resolve(chunks.join("")));
    process.stdin.on("error", reject);
  });
}

async function compileFromJsonStdin() {
  const raw = await readStdinUtf8();
  const payload = raw.trim() ? JSON.parse(raw) : {};
  const manifest = compileMdxToManifest(String(payload.source || ""), payload.options || {});
  process.stdout.write(`${JSON.stringify(manifest)}\n`);
}

function resolveCompileTargets(argv) {
  const input = argv[2];
  const output = argv[3];
  if (!input) {
    throw new Error(
      "usage: node compile-presentation.mjs <input.presentation.mdx|directory> [output.presentation.json|output-directory]",
    );
  }
  const inputPath = path.resolve(input);
  if (!fs.existsSync(inputPath)) {
    throw new Error(`input not found: ${inputPath}`);
  }
  const stat = fs.statSync(inputPath);
  if (stat.isDirectory()) {
    const files = collectPresentationFiles(inputPath);
    if (!files.length) {
      throw new Error(`no .presentation.mdx files found under ${inputPath}`);
    }
    const outputRoot = output ? path.resolve(output) : null;
    return files.map((filePath) => {
      const relative = path.relative(inputPath, filePath);
      const outputPath = outputRoot
        ? path.join(outputRoot, relative.replace(/\.presentation\.mdx$/i, ".presentation.json"))
        : filePath.replace(/\.presentation\.mdx$/i, ".presentation.json");
      return { inputPath: filePath, outputPath };
    });
  }
  return [
    {
      inputPath,
      outputPath: output
        ? path.resolve(output)
        : inputPath.replace(/\.presentation\.mdx$/i, ".presentation.json"),
    },
  ];
}

function compileTarget(target) {
  const source = fs.readFileSync(target.inputPath, "utf8");
  const manifest = compileMdxToManifest(source, {
    id: path.basename(target.inputPath, ".presentation.mdx"),
  });
  if (!manifest.steps.length) {
    throw new Error(`no steps parsed from ${target.inputPath}`);
  }
  fs.mkdirSync(path.dirname(target.outputPath), { recursive: true });
  fs.writeFileSync(target.outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  process.stdout.write(
    `compiled ${target.inputPath} -> ${target.outputPath} (${manifest.steps.length} steps)\n`,
  );
}

async function main() {
  if (process.argv.includes("--stdin-json")) {
    await compileFromJsonStdin();
    return;
  }
  const targets = resolveCompileTargets(process.argv);
  targets.forEach(compileTarget);
}

await main();
