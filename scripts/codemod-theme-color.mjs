#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const ROOT = path.join(import.meta.dirname, "..", "stock", "components");

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === "vendor") continue;
      walk(full, out);
    } else if (entry.name.endsWith(".js")) {
      out.push(full);
    }
  }
  return out;
}

const themeColorRe =
  /themeColor\(\s*(['"`])([a-zA-Z0-9_]+)\1\s*,\s*(['"`])[^'"`]+\3\s*\)/g;

for (const file of walk(ROOT)) {
  let src = fs.readFileSync(file, "utf8");
  if (!src.includes("themeColor(")) continue;
  const next = src.replace(themeColorRe, 'color("$2")');
  if (next === src) continue;
  if (!next.includes('from "../mei/theme-style.js"') && !next.includes('from "./theme-style.js"')) {
    if (next.includes('from "../cockpit/tokens.js"')) {
      src = next.replace(
        /import \{([^}]+)\} from "\.\.\/cockpit\/tokens\.js";/,
        (m, imports) => {
          const parts = imports.split(",").map((s) => s.trim()).filter(Boolean);
          const filtered = parts.filter((p) => p !== "themeColor" && p !== "themeColor as color");
          const importLine =
            filtered.length > 0
              ? `import { ${filtered.join(", ")} } from "../cockpit/tokens.js";\n`
              : "";
          return `${importLine}import { color } from "../mei/theme-style.js";`;
        },
      );
    } else if (next.includes('from "./tokens.js"')) {
      src = next.replace(
        /import \{([^}]+)\} from "\.\/tokens\.js";/,
        (m, imports) => {
          const parts = imports.split(",").map((s) => s.trim()).filter(Boolean);
          const filtered = parts.filter((p) => p !== "themeColor");
          const importLine =
            filtered.length > 0
              ? `import { ${filtered.join(", ")} } from "./tokens.js";\n`
              : "";
          return `${importLine}import { color } from "../mei/theme-style.js";`;
        },
      );
    } else {
      src = `import { color } from "../mei/theme-style.js";\n${next}`;
    }
  } else {
    src = next;
  }
  fs.writeFileSync(file, src);
  console.log("codemod", path.relative(ROOT, file));
}
