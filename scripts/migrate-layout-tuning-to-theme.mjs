#!/usr/bin/env node
/**
 * Migrate ops.layoutTuning entries into ops.themes.*.layout (L5b-3 prep).
 * Usage: node scripts/migrate-layout-tuning-to-theme.mjs --app data-demo [--write]
 */
import fs from "node:fs/promises";
import path from "node:path";

const args = process.argv.slice(2);
const appId = args.includes("--app") ? args[args.indexOf("--app") + 1] : "";
const write = args.includes("--write");
const workspaceRoot = process.env.MEI_WORKSPACE_ROOT || path.resolve("workspaces");

if (!appId) {
  console.error("usage: node migrate-layout-tuning-to-theme.mjs --app <appId> [--write]");
  process.exit(1);
}

async function main() {
  const configPath = path.join(workspaceRoot, appId, "mei.config.json");
  const raw = await fs.readFile(configPath, "utf8");
  const config = JSON.parse(raw);
  const tuning = config?.ops?.layoutTuning || config?.ops?.layout_tuning || {};
  const themes = config?.ops?.themes || {};
  const themeKey = themes.default ? "default" : Object.keys(themes)[0] || "default";
  if (!themes[themeKey]) themes[themeKey] = {};
  if (!themes[themeKey].layout) themes[themeKey].layout = {};

  const migrated = {};
  for (const [scope, patch] of Object.entries(tuning)) {
    if (!patch || typeof patch !== "object") continue;
    themes[themeKey].layout[scope] = { ...(themes[themeKey].layout[scope] || {}), ...patch };
    migrated[scope] = patch;
  }

  const report = {
    appId,
    themeKey,
    migratedScopes: Object.keys(migrated),
    write,
  };
  console.log(JSON.stringify(report, null, 2));

  if (!write || Object.keys(migrated).length === 0) return;

  config.ops = config.ops || {};
  config.ops.themes = themes;
  delete config.ops.layoutTuning;
  delete config.ops.layout_tuning;
  await fs.writeFile(configPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");
  console.log(`wrote ${configPath}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
