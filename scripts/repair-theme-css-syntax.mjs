#!/usr/bin/env node
/**
 * Repair broken var()/gradient syntax introduced by materialize-shell-css-literals.mjs.
 * Revert body-mounted scene overlays to --mei-color-* / --mei-font-*.
 */
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const appShellPath = path.join(root, "host-shell/app/assets/app-shell.css");

let css = fs.readFileSync(appShellPath, "utf8");

function fixGradients(text) {
  let out = text;
  // background: var(--a), var(--b));
  out = out.replace(
    /background:\s*var\((--mei-[\w-]+)\),\s*var\((--mei-[\w-]+)\)\);/gi,
    "background: linear-gradient(180deg, var($1), var($2));",
  );
  // indented continuation (multi-line background)
  out = out.replace(
    /^(\s*)var\((--mei-[\w-]+)\),\s*var\((--mei-[\w-]+)\)\);/gim,
    "$1linear-gradient(180deg, var($2), var($3));",
  );
  // background: var(--a) 0%, var(--b) 100%);
  out = out.replace(
    /background:\s*var\((--mei-[\w-]+)\)\s+0%,\s*var\((--mei-[\w-]+)\)\s+100%\);/gi,
    "background: linear-gradient(180deg, var($1) 0%, var($2) 100%);",
  );
  // broken multi-line drilldown: var(--a)), var(--b)) )
  out = out.replace(
    /background:\s*linear-gradient\(\s*\n\s*180deg,\s*\n\s*var\((--mei-color-[^)]+)\)\),\s*\n\s*var\((--mei-color-[^)]+)\)\)\s*\n\s*\);/g,
    "background: linear-gradient(180deg, var($1), var($2));",
  );
  // missing closing paren on linear-gradient(..., var(--b);
  out = out.replace(
    /linear-gradient\(180deg, var\((--mei-[\w-]+)\), var\((--mei-[\w-]+)\);/g,
    "linear-gradient(180deg, var($1), var($2));",
  );
  return out;
}

function fixVarParens(text) {
  return text.replace(/var\((--mei-[\w-]+)\)\);/gi, "var($1);");
}

css = fixGradients(css);
css = fixVarParens(css);
css = css.replace(
  /\.mei-surface-panel-muted\s*\{\s*background:\s*var\((--mei-[^)]+)\)\s+45%,\s*transparent\)\);/,
  ".mei-surface-panel-muted { background: color-mix(in srgb, var($1) 45%, transparent);",
);

const marker = "/* page-flow";
const splitAt = css.indexOf(marker);
if (splitAt === -1) throw new Error("page-flow marker missing");
let shell = css.slice(0, splitAt);
const scene = css.slice(splitAt);

const sceneOverlayStart =
  /^(\s*)(body\s*>\s*\.maplibregl|body\s*>\s*\.mei-cockpit|\.access-chat|\.access-drilldown|\.access-scene-board)/;
let overlayDepth = 0;

shell = shell
  .split("\n")
  .map((line) => {
    const trimmed = line.trim();
    if (sceneOverlayStart.test(line) && line.includes("{")) {
      overlayDepth = 1;
    } else if (overlayDepth > 0) {
      overlayDepth += (line.match(/\{/g) || []).length;
      overlayDepth -= (line.match(/\}/g) || []).length;
      if (overlayDepth <= 0) overlayDepth = 0;
    }
    if (overlayDepth > 0 || sceneOverlayStart.test(line)) {
      return line
        .replace(/--mei-shell-color-/g, "--mei-color-")
        .replace(/--mei-shell-font-/g, "--mei-font-")
        .replace(/--mei-shell-text\b/g, "--mei-color-text-primary")
        .replace(/--mei-shell-bg\b/g, "--mei-color-surface-bg");
    }
    return line;
  })
  .join("\n");

shell = fixGradients(shell);
shell = fixVarParens(shell);
let sceneFixed = fixGradients(scene);
sceneFixed = fixVarParens(sceneFixed);

let merged = shell + sceneFixed;
merged = merged.replace(
  /linear-gradient\(180deg, var\((--mei-[\w-]+)\), var\((--mei-[\w-]+)\);/g,
  "linear-gradient(180deg, var($1), var($2));",
);

fs.writeFileSync(appShellPath, merged);

const bad = (fs.readFileSync(appShellPath, "utf8").match(/var\(--mei-[^)]+\)\);/g) || [])
  .length;
console.log(`repaired app-shell.css (remaining bad paren: ${bad})`);
