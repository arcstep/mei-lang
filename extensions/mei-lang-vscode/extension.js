"use strict";

const fs = require("fs");
const path = require("path");
const vscode = require("vscode");
const {
  LanguageClient,
  TransportKind,
} = require("vscode-languageclient/node");

/** @type {import("vscode-languageclient/node").LanguageClient | undefined} */
let client;

/**
 * @param {string} startDir
 * @returns {string[]}
 */
function collectAncestorDirs(startDir) {
  const dirs = [];
  let current = path.resolve(startDir);
  for (;;) {
    dirs.push(current);
    const parent = path.dirname(current);
    if (parent === current) break;
    current = parent;
  }
  return dirs;
}

/**
 * @param {string} candidate
 * @returns {boolean}
 */
function isExecutableFile(candidate) {
  try {
    const st = fs.statSync(candidate);
    if (!st.isFile()) return false;
    fs.accessSync(candidate, fs.constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

/**
 * @param {string[]} bases
 * @returns {string | undefined}
 */
function findMeiLspNear(bases) {
  const relativeCandidates = [
    path.join(".mei", "runtime", "bin", "mei-lsp"),
    path.join("mei-lang", ".mei", "runtime", "bin", "mei-lsp"),
    path.join("mei-lang", "target", "debug", "mei-lsp"),
    path.join("target", "debug", "mei-lsp"),
    path.join("mei-lang", "target", "release", "mei-lsp"),
    path.join("target", "release", "mei-lsp"),
  ];

  for (const base of bases) {
    for (const rel of relativeCandidates) {
      const candidate = path.join(base, rel);
      if (isExecutableFile(candidate)) return candidate;
    }
  }
  return undefined;
}

/**
 * @returns {string | undefined}
 */
function findMeiLspOnPath() {
  const pathEnv = process.env.PATH || "";
  const parts = pathEnv.split(path.delimiter).filter(Boolean);
  const name = process.platform === "win32" ? "mei-lsp.exe" : "mei-lsp";
  for (const dir of parts) {
    const candidate = path.join(dir, name);
    if (isExecutableFile(candidate)) return candidate;
  }
  return undefined;
}

/**
 * @returns {string | undefined}
 */
function resolveMeiLspCommand() {
  const configured = vscode.workspace
    .getConfiguration("mei")
    .get("lsp.path", "");
  if (typeof configured === "string" && configured.trim()) {
    const absolute = path.resolve(configured.trim());
    if (isExecutableFile(absolute)) return absolute;
    vscode.window.showWarningMessage(
      `MeiLang: mei.lsp.path 不可执行：${absolute}`
    );
  }

  /** @type {string[]} */
  const searchRoots = [];
  for (const folder of vscode.workspace.workspaceFolders || []) {
    searchRoots.push(...collectAncestorDirs(folder.uri.fsPath));
  }
  // Extension lives at mei-lang/extensions/mei-lang-vscode → mei-lang root is ../..
  searchRoots.push(
    ...collectAncestorDirs(path.resolve(__dirname, "..", ".."))
  );

  const unique = [...new Set(searchRoots)];
  const near = findMeiLspNear(unique);
  if (near) return near;

  return findMeiLspOnPath();
}

/**
 * @param {import("vscode").ExtensionContext} context
 */
async function activate(context) {
  const command = resolveMeiLspCommand();
  if (!command) {
    vscode.window.showWarningMessage(
      "MeiLang: 未找到 mei-lsp。着色仍可用。请 cargo build -p mei-lang-lsp，或设置 mei.lsp.path。"
    );
    return;
  }

  const serverOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio },
  };

  /** @type {import("vscode-languageclient").LanguageClientOptions} */
  const clientOptions = {
    documentSelector: [{ scheme: "file", language: "mei" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.mei"),
    },
  };

  client = new LanguageClient(
    "mei-lsp",
    "MeiLang Language Server",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push(client);
  await client.start();
}

async function deactivate() {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

module.exports = { activate, deactivate };
