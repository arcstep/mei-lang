#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const view = fs.readFileSync(
  path.join(root, "host-shell/app/src/ui/topbar/view/view.rs"),
  "utf8",
);
const routing = fs.readFileSync(
  path.join(root, "host-shell/app/src/ui/view_routing.rs"),
  "utf8",
);
const shell = fs.readFileSync(
  path.join(root, "host-shell/app/src/ui/shell_workspace.rs"),
  "utf8",
);

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

assert(view.includes("show_workspace_share"), "topbar must gate 资料交换 by capability");
assert(view.includes('"资料交换"'), "topbar must render 资料交换 label");
assert(view.includes('"应用中心"'), "topbar must still render 应用中心");
assert(routing.includes("host_share_href"), "share href helper must exist");
assert(shell.includes("WorkspaceShellNav::Share"), "shell nav must include Share");
assert(
  view.indexOf('"资料交换"') < view.indexOf('"应用中心"'),
  "资料交换 must appear before 应用中心 in shell_nav_view",
);
console.log("host-topbar-share-nav.test: ok");
