#!/usr/bin/env node
/** Restore body-mounted scene overlay CSS from pre-split git, strip var() fallbacks only. */
import fs from "node:fs";
import path from "node:path";
import { execSync } from "node:child_process";

const root = path.resolve(import.meta.dirname, "..");
const appShellPath = path.join(root, "app/assets/app-shell.css");

function stripFallbacks(text) {
  let out = "";
  let i = 0;
  while (i < text.length) {
    const idx = text.indexOf("var(--mei-", i);
    if (idx === -1) {
      out += text.slice(i);
      break;
    }
    out += text.slice(i, idx);
    const open = idx + 3; // points at '('
    let depth = 0;
    let j = open;
    for (; j < text.length; j++) {
      const ch = text[j];
      if (ch === "(") depth++;
      else if (ch === ")") {
        depth--;
        if (depth === 0) {
          j++;
          break;
        }
      }
    }
    const inner = text.slice(open + 1, j - 1);
    const comma = inner.indexOf(",");
    const varName = (comma === -1 ? inner : inner.slice(0, comma)).trim();
    out += `var(${varName})`;
    i = j;
  }
  return out;
}

const orig = execSync("git show 45a8f77^:app/assets/app-shell.css", {
  cwd: root,
  encoding: "utf8",
});
const origShell = orig.slice(0, orig.indexOf("/* page-flow"));
const start = origShell.indexOf(".access-chat-floating-root");
const end = origShell.indexOf(".access-drilldown-overlay--size-comfortable");
if (start === -1 || end === -1) throw new Error("overlay block not found in git");
const overlayCss = stripFallbacks(origShell.slice(start, end));

let css = fs.readFileSync(appShellPath, "utf8");
const shellEnd = css.indexOf("/* page-flow");
let shell = css.slice(0, shellEnd);
const scene = css.slice(shellEnd);

const curStart = shell.indexOf(".access-chat-floating-root");
const curEnd = shell.indexOf(".access-drilldown-overlay--size-comfortable");
if (curStart === -1 || curEnd === -1) throw new Error("overlay block not found in current");
shell = shell.slice(0, curStart) + overlayCss + shell.slice(curEnd);

fs.writeFileSync(appShellPath, shell + scene);
console.log("restored scene overlay CSS from git baseline (no fallbacks)");
