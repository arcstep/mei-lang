#!/usr/bin/env node
/**
 * 将 `.presentation.mdx` 编译为 `*.presentation.json`（PresentationManifest IR）。
 * 浏览器运行时只读取 JSON，不解析 MDX。
 */
import fs from "node:fs";
import path from "node:path";

function parseFrontmatter(source) {
  const match = source.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) return { meta: {}, body: source };
  const meta = {};
  for (const line of match[1].split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const idx = trimmed.indexOf(":");
    if (idx < 0) continue;
    const key = trimmed.slice(0, idx).trim();
    const value = trimmed.slice(idx + 1).trim();
    if (key) meta[key] = value;
  }
  return { meta, body: source.slice(match[0].length) };
}

function parseStepBlock(headingLine, lines) {
  const headingMatch = headingLine.match(/^##\s+(.+?)(?:\s+\{#([^}]+)\})?\s*$/);
  if (!headingMatch) return null;
  const step = {
    id: String(headingMatch[2] || headingMatch[1] || "").trim(),
    title: String(headingMatch[1] || "").trim(),
  };
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const idx = trimmed.indexOf(":");
    if (idx < 0) continue;
    const key = trimmed.slice(0, idx).trim();
    const value = trimmed.slice(idx + 1).trim();
    if (!key || !value) continue;
    if (key === "composition") step.composition = value;
    else if (key === "caption") step.caption = value;
    else if (key === "speaker_notes") step.speaker_notes = value;
    else if (key === "slide") step.slide = { document: value };
    else if (key === "cockpit_scene") {
      step.cockpit = step.cockpit || { scene: value, actions: [] };
      step.cockpit.scene = value;
    } else if (key === "highlight") {
      step.cockpit = step.cockpit || { scene: "home", actions: [] };
      step.cockpit.actions.push({ type: "highlight", viewpoint: value });
    } else if (key === "open_t2_page" || key === "open_board") {
      const parts = value.split(/\s+/).filter(Boolean);
      step.cockpit = step.cockpit || { scene: "home", actions: [] };
      const action = { type: "open_t2_page", pageSceneId: parts[0] };
      if (parts[1]) action.projection = parts[1];
      step.cockpit.actions.push(action);
    } else if (key === "binding") step.cockpit = { binding: value, scene: "home", actions: [] };
  }
  if (!step.id) return null;
  return step;
}

function markdownToHtml(markdown) {
  const lines = String(markdown || "").split(/\r?\n/);
  const parts = [];
  let paragraph = [];
  const flush = () => {
    if (!paragraph.length) return;
    parts.push(`<p>${paragraph.join(" ")}</p>`);
    paragraph = [];
  };
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) {
      flush();
      continue;
    }
    if (line.startsWith("# ")) {
      flush();
      parts.push(`<h1>${line.slice(2)}</h1>`);
      continue;
    }
    if (line.startsWith("## ")) {
      flush();
      parts.push(`<h2>${line.slice(3)}</h2>`);
      continue;
    }
    paragraph.push(line);
  }
  flush();
  return parts.join("");
}

function parseBindingActions(bindingSource) {
  const actions = [];
  const highlightRe = /highlight\s*\(\s*viewpoint\s*=\s*"([^"]+)"/g;
  let match = highlightRe.exec(bindingSource);
  while (match) {
    actions.push({ type: "highlight", viewpoint: match[1] });
    match = highlightRe.exec(bindingSource);
  }
  const openT2PageRe =
    /open_t2_page\s*\(\s*page_scene_id\s*=\s*"([^"]+)"(?:\s*,\s*projection\s*=\s*"([^"]+)")?/g;
  match = openT2PageRe.exec(bindingSource);
  while (match) {
    const action = { type: "open_t2_page", pageSceneId: match[1] };
    if (match[2]) action.projection = match[2];
    actions.push(action);
    match = openT2PageRe.exec(bindingSource);
  }
  return actions;
}

function enrichStep(step, presentationDir) {
  const appSrcDir = path.resolve(presentationDir, "..");
  if (step.slide?.document) {
    const slidePath = path.resolve(appSrcDir, step.slide.document);
    if (fs.existsSync(slidePath)) {
      const markdown = fs.readFileSync(slidePath, "utf8");
      step.slide.html = markdownToHtml(markdown);
    }
  }
  if (step.cockpit?.binding) {
    const bindingPath = path.resolve(presentationDir, step.cockpit.binding);
    if (fs.existsSync(bindingPath)) {
      const bindingSource = fs.readFileSync(bindingPath, "utf8");
      step.cockpit.actions = parseBindingActions(bindingSource);
      const sceneMatch = bindingSource.match(/scene\s*=\s*"([^"]+)"/);
      if (sceneMatch) step.cockpit.scene = sceneMatch[1];
    }
  }
  return step;
}

function compileMdxToManifest(source, baseDir, defaults = {}) {
  const { meta, body } = parseFrontmatter(source);
  const manifest = {
    id: String(meta.presentation || defaults.id || "intro").trim(),
    title: String(meta.title || defaults.title || "").trim(),
    steps: [],
  };
  const chunks = body.split(/^##\s+/m).filter(Boolean);
  for (const chunk of chunks) {
    const lines = chunk.split(/\r?\n/);
    const heading = `## ${lines[0]}`;
    const step = parseStepBlock(heading, lines.slice(1));
    if (step) manifest.steps.push(enrichStep(step, baseDir));
  }
  return manifest;
}

function resolvePaths(argv) {
  const input = argv[2];
  const output = argv[3];
  if (!input) {
    throw new Error("usage: node compile-presentation.mjs <input.presentation.mdx> [output.presentation.json]");
  }
  const inputPath = path.resolve(input);
  const outputPath = output
    ? path.resolve(output)
    : inputPath.replace(/\.presentation\.mdx$/i, ".presentation.json");
  return { inputPath, outputPath };
}

function main() {
  const { inputPath, outputPath } = resolvePaths(process.argv);
  const source = fs.readFileSync(inputPath, "utf8");
  const manifest = compileMdxToManifest(source, path.dirname(inputPath));
  if (!manifest.steps.length) {
    throw new Error(`no steps parsed from ${inputPath}`);
  }
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  process.stdout.write(`compiled ${inputPath} -> ${outputPath} (${manifest.steps.length} steps)\n`);
}

main();
