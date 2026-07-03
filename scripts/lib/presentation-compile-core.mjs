import { marked } from "marked";

const STEP_HEADING_RE = /^##\s+(.+?)(?:\s+\{#([^}]+)\})?\s*$/;
const BLOCK_END_RE = /^@end\s*$/;
const SUPPORTED_COMPOSITIONS = new Set([
  "slides_only",
  "cockpit_only",
  "slides_over_cockpit",
]);
const SUPPORTED_PLANES = new Set(["t0", "t1", "t2"]);
const SLOT_EMBED_DIRECTIVES = new Set(["metric", "chart", "image", "embed"]);

export function parseFrontmatter(source) {
  const match = source.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) return { meta: {}, body: source };
  const meta = {};
  for (const line of match[1].split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const idx = trimmed.indexOf(":");
    if (idx < 0) continue;
    const key = trimmed.slice(0, idx).trim();
    const value = trimMatchingQuotes(trimmed.slice(idx + 1).trim());
    if (key) meta[key] = value;
  }
  return { meta, body: source.slice(match[0].length) };
}

export function trimMatchingQuotes(value) {
  const text = String(value || "").trim();
  if (
    (text.startsWith('"') && text.endsWith('"')) ||
    (text.startsWith("'") && text.endsWith("'"))
  ) {
    return text.slice(1, -1);
  }
  return text;
}

export function markdownToHtml(markdown) {
  const html = marked.parse(String(markdown || ""), { async: false });
  return typeof html === "string" ? html.trim() : "";
}

export function htmlToPlainText(html) {
  return String(html || "")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/\s+/g, " ")
    .trim();
}

export function markdownToPlainText(markdown) {
  return htmlToPlainText(markdownToHtml(markdown));
}

export function escapeHtml(value) {
  return String(value || "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function sanitizeClassToken(value, fallback = "default") {
  const token = String(value || "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/^-+|-+$/g, "");
  return token || fallback;
}

export function splitDirectiveArgs(source) {
  const parts = [];
  let current = "";
  let quote = "";
  let depth = 0;
  for (const ch of String(source || "")) {
    if (quote) {
      current += ch;
      if (ch === quote) quote = "";
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      current += ch;
      continue;
    }
    if (ch === "(" || ch === "[" || ch === "{") {
      depth += 1;
      current += ch;
      continue;
    }
    if (ch === ")" || ch === "]" || ch === "}") {
      depth = Math.max(0, depth - 1);
      current += ch;
      continue;
    }
    if (ch === "," && depth === 0) {
      const part = current.trim();
      if (part) parts.push(part);
      current = "";
      continue;
    }
    current += ch;
  }
  const tail = current.trim();
  if (tail) parts.push(tail);
  return parts.map(trimMatchingQuotes);
}

export function parseDirectiveLine(trimmed) {
  if (!trimmed.startsWith("@")) return null;
  const bare = trimmed.match(/^@([A-Za-z_][\w]*)\s*$/);
  if (bare) {
    return { name: bare[1], args: [] };
  }
  const invoked = trimmed.match(/^@([A-Za-z_][\w]*)\((.*)\)\s*$/);
  if (!invoked) return null;
  return { name: invoked[1], args: splitDirectiveArgs(invoked[2]) };
}

export function isBlockDirective(trimmed) {
  if (trimmed === "@caption" || trimmed === "@speaker_notes" || trimmed === "@speakerNotes") {
    return true;
  }
  return /^@slot\((.+)\)\s*$/.test(trimmed);
}

export function splitStepChunks(body) {
  const lines = String(body || "").split(/\r?\n/);
  const chunks = [];
  let current = null;
  let activeBlock = false;
  for (const raw of lines) {
    if (!current) {
      if (!raw.trim()) continue;
      if (STEP_HEADING_RE.test(raw)) {
        current = { headingLine: raw, lines: [] };
        continue;
      }
      throw new Error(`content outside step heading is not allowed: ${raw.trim()}`);
    }
    const trimmed = raw.trim();
    if (!activeBlock && STEP_HEADING_RE.test(raw)) {
      chunks.push(current);
      current = { headingLine: raw, lines: [] };
      continue;
    }
    current.lines.push(raw);
    if (!activeBlock && isBlockDirective(trimmed)) {
      activeBlock = true;
      continue;
    }
    if (activeBlock && BLOCK_END_RE.test(trimmed)) {
      activeBlock = false;
    }
  }
  if (activeBlock) {
    throw new Error(`unterminated block in step ${current?.headingLine || ""}`.trim());
  }
  if (current) chunks.push(current);
  return chunks;
}

export function collectBlock(lines, startIndex, label) {
  const content = [];
  for (let idx = startIndex; idx < lines.length; idx += 1) {
    if (BLOCK_END_RE.test(lines[idx].trim())) {
      return {
        content: content.join("\n").trim(),
        nextIndex: idx,
      };
    }
    content.push(lines[idx]);
  }
  throw new Error(`unterminated block for ${label}`);
}

export function normalizeComposition(value, fallback = "") {
  const normalized = String(value || fallback || "").trim();
  if (!normalized) return "";
  if (!SUPPORTED_COMPOSITIONS.has(normalized)) {
    throw new Error(`unsupported composition: ${normalized}`);
  }
  return normalized;
}

export function normalizePlane(value) {
  const normalized = String(value || "").trim().toLowerCase();
  if (!SUPPORTED_PLANES.has(normalized)) {
    throw new Error(`unsupported plane: ${value}`);
  }
  return normalized;
}

export function createEmbed(kind, args) {
  const positional = args.map(trimMatchingQuotes).filter(Boolean);
  const ref = positional[0] || "";
  if (!ref) {
    throw new Error(`@${kind} requires a reference id`);
  }
  return {
    type: "embed",
    kind,
    ref,
    html: renderEmbedHtml(kind, ref),
  };
}

export function renderEmbedHtml(kind, ref) {
  const labelMap = {
    metric: "Metric",
    chart: "Chart",
    image: "Image",
    embed: "Embed",
  };
  const label = labelMap[kind] || "Embed";
  return (
    `<div class="mei-presentation-embed mei-presentation-embed--${sanitizeClassToken(kind)}" ` +
    `data-embed-kind="${escapeHtml(kind)}" data-embed-ref="${escapeHtml(ref)}">` +
    `<span class="mei-presentation-embed-label">${label}</span>` +
    `<strong class="mei-presentation-embed-ref">${escapeHtml(ref)}</strong>` +
    `</div>`
  );
}

export function flushMarkdownSegment(buffer, segments) {
  const markdown = buffer.join("\n").trim();
  if (!markdown) {
    buffer.length = 0;
    return;
  }
  const html = markdownToHtml(markdown);
  segments.push({
    type: "markdown",
    markdown,
    html,
  });
  buffer.length = 0;
}

export function parseSlot(name, content) {
  const lines = String(content || "").split(/\r?\n/);
  const segments = [];
  const embeds = [];
  const markdownBuffer = [];
  for (const raw of lines) {
    const trimmed = raw.trim();
    const directive = parseDirectiveLine(trimmed);
    if (directive && SLOT_EMBED_DIRECTIVES.has(directive.name)) {
      flushMarkdownSegment(markdownBuffer, segments);
      const embed = createEmbed(directive.name, directive.args);
      embeds.push(embed);
      segments.push(embed);
      continue;
    }
    markdownBuffer.push(raw);
  }
  flushMarkdownSegment(markdownBuffer, segments);
  return {
    name,
    markdown: String(content || "").trim(),
    html: segments.map((segment) => segment.html || "").join(""),
    embeds,
    segments,
  };
}

export function renderSlideFromLayout(layoutId, slots) {
  const layout = sanitizeClassToken(layoutId, "stack");
  const slotMap = new Map(Array.isArray(slots) ? slots.map((slot) => [slot.name, slot]) : []);
  const renderSlot = (name, fallbackTag = "section") => {
    const slot = slotMap.get(name);
    const html = slot?.html || "";
    if (!html) return "";
    const tag = fallbackTag;
    return `<${tag} class="mei-presentation-slot mei-presentation-slot--${sanitizeClassToken(name)}" data-slot="${escapeHtml(name)}">${html}</${tag}>`;
  };
  if (layout === "title-and-evidence") {
    return (
      `<article class="mei-presentation-layout mei-presentation-layout--${layout}" data-layout="${escapeHtml(layoutId)}">` +
      `<div class="mei-presentation-layout-grid">` +
      `<header class="mei-presentation-layout-head">${renderSlot("title", "div")}</header>` +
      `<section class="mei-presentation-layout-body">${renderSlot("body", "div")}${renderSlot("support", "div")}</section>` +
      `<aside class="mei-presentation-layout-evidence">${renderSlot("evidence", "div")}</aside>` +
      `</div>` +
      `</article>`
    );
  }
  const generic = (Array.isArray(slots) ? slots : [])
    .map(
      (slot) =>
        `<section class="mei-presentation-slot mei-presentation-slot--${sanitizeClassToken(slot.name)}" data-slot="${escapeHtml(slot.name)}">${slot.html || ""}</section>`,
    )
    .join("");
  return `<article class="mei-presentation-layout mei-presentation-layout--${layout}" data-layout="${escapeHtml(layoutId)}">${generic}</article>`;
}

export function parseActionDirective(directive) {
  const name = directive.name;
  if (name === "showPlane") {
    return { type: "show_plane", plane: normalizePlane(directive.args[0]) };
  }
  if (name === "hidePlane") {
    return { type: "hide_plane", plane: normalizePlane(directive.args[0]) };
  }
  if (name === "highlight") {
    const viewpoint = String(directive.args[0] || "").trim();
    if (!viewpoint) throw new Error("@highlight requires a viewpoint id");
    return { type: "highlight", viewpoint };
  }
  if (name === "cameraMove") {
    const viewpoint = String(directive.args[0] || "").trim();
    if (!viewpoint) throw new Error("@cameraMove requires a viewpoint id");
    return { type: "camera_move", viewpoint };
  }
  if (name === "focusEntity") {
    const viewpoint = String(directive.args[0] || "").trim();
    if (!viewpoint) throw new Error("@focusEntity requires a viewpoint id");
    return { type: "focus_entity", viewpoint };
  }
  if (name === "showGroup") {
    const viewpoint = String(directive.args[0] || "").trim();
    if (!viewpoint) throw new Error("@showGroup requires a viewpoint id");
    return { type: "show_group", viewpoint };
  }
  if (name === "hideGroup") {
    const viewpoint = String(directive.args[0] || "").trim();
    if (!viewpoint) throw new Error("@hideGroup requires a viewpoint id");
    return { type: "hide_group", viewpoint };
  }
  if (name === "enterWorldView") {
    const viewpoint = String(directive.args[0] || "").trim();
    if (!viewpoint) throw new Error("@enterWorldView requires a viewpoint id");
    return { type: "enter_world_view", viewpoint };
  }
  if (name === "exitWorldView") {
    const viewpoint = String(directive.args[0] || "").trim();
    if (!viewpoint) throw new Error("@exitWorldView requires a viewpoint id");
    return { type: "exit_world_view", viewpoint };
  }
  if (name === "cutawayToggle") {
    const viewpoint = String(directive.args[0] || "").trim();
    if (!viewpoint) throw new Error("@cutawayToggle requires a viewpoint id");
    return { type: "cutaway_toggle", viewpoint };
  }
  if (name === "openT2Page" || name === "openBoard") {
    const pageSceneId = String(directive.args[0] || "").trim();
    if (!pageSceneId) throw new Error(`@${name} requires a page scene id`);
    const action = { type: "open_t2_page", pageSceneId };
    const projection = String(directive.args[1] || "").trim();
    if (projection) action.projection = projection;
    return action;
  }
  return null;
}

export function parseStepChunk(chunk, defaults = {}) {
  const headingMatch = chunk.headingLine.match(STEP_HEADING_RE);
  if (!headingMatch) {
    throw new Error(`invalid step heading: ${chunk.headingLine}`);
  }
  const title = String(headingMatch[1] || "").trim();
  const step = {
    id: String(headingMatch[2] || title).trim(),
    title,
    composition: normalizeComposition(defaults.defaultComposition || "", ""),
  };
  const actions = [];
  const slots = [];
  let layoutId = String(defaults.defaultLayout || "").trim();
  for (let idx = 0; idx < chunk.lines.length; idx += 1) {
    const raw = chunk.lines[idx];
    const trimmed = raw.trim();
    if (!trimmed) continue;
    const directive = parseDirectiveLine(trimmed);
    if (!directive) {
      throw new Error(`unexpected content outside directive in step ${step.id}: ${trimmed}`);
    }
    if (directive.name === "caption") {
      const block = collectBlock(chunk.lines, idx + 1, "@caption");
      step.captionMarkdown = block.content;
      step.captionHtml = markdownToHtml(block.content);
      step.caption = markdownToPlainText(block.content);
      idx = block.nextIndex;
      continue;
    }
    if (directive.name === "speaker_notes" || directive.name === "speakerNotes") {
      const block = collectBlock(chunk.lines, idx + 1, "@speaker_notes");
      step.speakerNotesMarkdown = block.content;
      step.speakerNotesHtml = markdownToHtml(block.content);
      step.speaker_notes = markdownToPlainText(block.content);
      idx = block.nextIndex;
      continue;
    }
    if (directive.name === "slot") {
      const slotName = String(directive.args[0] || "").trim();
      if (!slotName) throw new Error("@slot requires a slot name");
      const block = collectBlock(chunk.lines, idx + 1, `@slot(${slotName})`);
      slots.push(parseSlot(slotName, block.content));
      idx = block.nextIndex;
      continue;
    }
    if (directive.name === "composition") {
      step.composition = normalizeComposition(directive.args[0], defaults.defaultComposition);
      continue;
    }
    if (directive.name === "layout") {
      layoutId = String(directive.args[0] || "").trim();
      if (!layoutId) throw new Error("@layout requires a layout id");
      continue;
    }
    const action = parseActionDirective(directive);
    if (action) {
      actions.push(action);
      continue;
    }
    throw new Error(`unsupported directive in step ${step.id}: @${directive.name}`);
  }
  if (!step.composition) {
    if (slots.length && actions.length) step.composition = "slides_over_cockpit";
    else if (slots.length) step.composition = "slides_only";
    else step.composition = "cockpit_only";
  }
  if (slots.length || layoutId) {
    const effectiveLayout = layoutId || "stack";
    step.slide = {
      layout: effectiveLayout,
      slots,
    };
    step.slide.html = renderSlideFromLayout(effectiveLayout, slots);
  }
  if (actions.length) {
    step.actions = actions;
    step.cockpit = {
      scene: String(defaults.defaultScene || "home").trim() || "home",
      actions: actions.slice(),
    };
  }
  return step;
}

export function compileMdxToManifest(source, options = {}) {
  const defaults = options && typeof options === "object" ? options : {};
  const { meta, body } = parseFrontmatter(source);
  const manifest = {
    id: String(meta.presentation || defaults.id || "ephemeral").trim(),
    title: String(meta.title || defaults.title || "").trim(),
    defaultLayout: String(meta.default_layout || defaults.defaultLayout || "").trim(),
    defaultComposition: normalizeComposition(
      meta.default_composition || defaults.defaultComposition || "",
      "",
    ),
    steps: [],
  };
  const stepChunks = splitStepChunks(body);
  for (const chunk of stepChunks) {
    const step = parseStepChunk(chunk, {
      defaultLayout: manifest.defaultLayout,
      defaultComposition: manifest.defaultComposition,
      defaultScene: defaults.defaultScene || "home",
    });
    manifest.steps.push(step);
  }
  return manifest;
}
