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
/** @type {import("vscode").DiagnosticCollection | undefined} */
let adminDiagnostics;

const ADMIN_FRONTMATTER_FIELDS = new Set([
  "api_version",
  "title",
  "description",
  "menu",
  "parent",
  "order",
  "keywords",
  "default",
  "required_capabilities",
  "scope",
  "audit",
  "danger_level",
]);

function adminDiagnostic(document, line, message, code) {
  const range = document.lineAt(Math.max(0, line)).range;
  const diagnostic = new vscode.Diagnostic(
    range,
    message,
    vscode.DiagnosticSeverity.Error
  );
  diagnostic.source = "MeiLang Admin MDX";
  diagnostic.code = code;
  return diagnostic;
}

function validateAdminMdx(document) {
  if (document.languageId !== "mei-admin-mdx") return;
  const lines = document.getText().split(/\r?\n/);
  const diagnostics = [];
  if (lines[0]?.trim() !== "---") {
    diagnostics.push(
      adminDiagnostic(document, 0, "Admin MDX 必须以 --- frontmatter 开始", "admin_mdx_parse")
    );
    adminDiagnostics.set(document.uri, diagnostics);
    return;
  }
  const end = lines.findIndex((line, index) => index > 0 && line.trim() === "---");
  if (end < 0) {
    diagnostics.push(
      adminDiagnostic(document, 0, "Admin MDX 缺少 frontmatter 结束 ---", "admin_mdx_parse")
    );
    adminDiagnostics.set(document.uri, diagnostics);
    return;
  }
  const values = new Map();
  for (let index = 1; index < end; index += 1) {
    const raw = lines[index].trim();
    if (!raw) continue;
    const separator = raw.indexOf(":");
    if (separator < 1) {
      diagnostics.push(
        adminDiagnostic(document, index, "frontmatter 必须使用 key: value", "admin_mdx_parse")
      );
      continue;
    }
    const key = raw.slice(0, separator).trim();
    const value = raw.slice(separator + 1).trim().replace(/^['"]|['"]$/g, "");
    if (!ADMIN_FRONTMATTER_FIELDS.has(key)) {
      diagnostics.push(
        adminDiagnostic(document, index, `未知 frontmatter 字段 ${key}`, "admin_mdx_parse")
      );
    } else if (values.has(key)) {
      diagnostics.push(
        adminDiagnostic(document, index, `重复 frontmatter 字段 ${key}`, "admin_mdx_parse")
      );
    } else {
      values.set(key, value);
    }
  }
  for (const required of [
    "api_version",
    "title",
    "required_capabilities",
  ]) {
    if (!values.get(required)) {
      diagnostics.push(
        adminDiagnostic(document, 0, `缺少必填 frontmatter 字段 ${required}`, "admin_mdx_parse")
      );
    }
  }
  if (
    values.has("api_version") &&
    values.get("api_version") !== "mei-admin-resource-v2"
  ) {
    diagnostics.push(
      adminDiagnostic(
        document,
        0,
        `不支持 api_version ${values.get("api_version")}`,
        "admin_api_version_unsupported"
      )
    );
  }
  const directives = /^@(scene|fill)\(.+\)$/;
  for (let index = end + 1; index < lines.length; index += 1) {
    const raw = lines[index].trim();
    if (!raw) continue;
    if (raw.startsWith("@") && !directives.test(raw)) {
      diagnostics.push(
        adminDiagnostic(document, index, `未知或格式错误的 Admin 指令 ${raw}`, "admin_mdx_parse")
      );
      continue;
    }
    const sectionHeading = /^##\s+.+\s+\{#[A-Za-z_][A-Za-z0-9_-]*\}$/.test(raw);
    if (!sectionHeading && (raw.includes("<") || raw.includes(">"))) {
      diagnostics.push(
        adminDiagnostic(document, index, "Admin MDX 禁止 JSX/HTML", "admin_mdx_jsx_forbidden")
      );
    }
    if (!sectionHeading && (raw.includes("{") || raw.includes("}"))) {
      diagnostics.push(
        adminDiagnostic(document, index, "Admin MDX 禁止 JavaScript 表达式", "admin_mdx_js_forbidden")
      );
    }
  }
  adminDiagnostics.set(document.uri, diagnostics);
}

function activateAdminDiagnostics(context) {
  adminDiagnostics = vscode.languages.createDiagnosticCollection("mei-admin-mdx");
  context.subscriptions.push(adminDiagnostics);
  for (const document of vscode.workspace.textDocuments) validateAdminMdx(document);
  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument(validateAdminMdx),
    vscode.workspace.onDidChangeTextDocument((event) => validateAdminMdx(event.document)),
    vscode.workspace.onDidCloseTextDocument((document) => adminDiagnostics.delete(document.uri))
  );
}

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
  activateAdminDiagnostics(context);
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
