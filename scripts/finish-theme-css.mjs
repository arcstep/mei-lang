#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const cssPath = path.resolve("app/assets/app-shell.css");
let css = fs.readFileSync(cssPath, "utf8");
const parts = css.split(/(:root\s*\{[\s\S]*?\})/m);
const root = parts[1] ?? "";
let body = (parts[0] ?? "") + (parts.slice(2).join("") || "");

body = body.replace(/color:\s*rgba\([^;)]+\)/gi, "color: var(--mei-color-text-muted)");
body = body.replace(/color:\s*#[0-9a-fA-F]{3,8}\b/gi, "color: var(--mei-color-text-primary)");
body = body.replace(/font-size:\s*(\d+)px\b/g, "font-size: var(--mei-font-2, $1px)");

fs.writeFileSync(cssPath, root + body);
console.log("finished theme css sweep");
