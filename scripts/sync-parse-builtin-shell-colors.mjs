#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const root = path.resolve(import.meta.dirname, "..");
const fragment = JSON.parse(
  fs.readFileSync(path.join(root, "scripts/workspace-host-theme.fragment.json"), "utf8"),
);
const color = fragment.themes.host.tokens.color;
const lines = Object.entries(color).map(
  ([k, v]) => `        "${k}": ${JSON.stringify(v)},`,
);
const block = `fn page_shell_color_tokens() -> Value {
    serde_json::json!({
${lines.join("\n")}
    })
}`;

const builtinPath = path.join(root, "app/src/ui/preview/theme/parse_builtin.rs");
let src = fs.readFileSync(builtinPath, "utf8");
src = src.replace(/fn page_shell_color_tokens\(\) -> Value \{[\s\S]*?\n\}/, block);
fs.writeFileSync(builtinPath, src);
console.log(`synced page_shell_color_tokens (${Object.keys(color).length} keys)`);
