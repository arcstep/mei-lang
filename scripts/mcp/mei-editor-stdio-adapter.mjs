#!/usr/bin/env node

import { spawn } from "node:child_process";

const MEI_BIN = process.env.MEI_BIN || "mei";
const DEFAULT_SOURCE_ROOT = process.env.MEI_SOURCE_ROOT || "";

const TOOL_DEFS = [
  {
    name: "mei_check",
    description: "Compile an app and return diagnostics plus revision metadata.",
    inputSchema: {
      type: "object",
      properties: {
        app: { type: "string" },
        source_root: { type: "string" },
        scene: { type: "string" },
        target_file: { type: "string" },
      },
      required: ["app"],
    },
  },
  {
    name: "mei_compile",
    description: "Compile an app and return the same JSON contract as check for scripted consumers.",
    inputSchema: {
      type: "object",
      properties: {
        app: { type: "string" },
        source_root: { type: "string" },
        scene: { type: "string" },
        target_file: { type: "string" },
      },
      required: ["app"],
    },
  },
  {
    name: "mei_host_describe",
    description: "Return machine-readable host runtime contract descriptor.",
    inputSchema: {
      type: "object",
      properties: {},
      additionalProperties: false,
    },
  },
  {
    name: "mei_inspect_world",
    description: "Return the structured world/runtime snapshot for the selected app scope.",
    inputSchema: {
      type: "object",
      properties: {
        app: { type: "string" },
        source_root: { type: "string" },
        scene: { type: "string" },
        target_file: { type: "string" },
      },
      required: ["app"],
    },
  },
  {
    name: "mei_inspect_inventory",
    description: "Return the app inventory/resource index for the selected scope.",
    inputSchema: {
      type: "object",
      properties: {
        app: { type: "string" },
        source_root: { type: "string" },
        scene: { type: "string" },
        target_file: { type: "string" },
      },
      required: ["app"],
    },
  },
  {
    name: "mei_query_dataset",
    description: "Run bounded dataset row/schema queries.",
    inputSchema: {
      type: "object",
      properties: {
        app: { type: "string" },
        source_root: { type: "string" },
        dataset_id: { type: "string" },
        scene: { type: "string" },
        target_file: { type: "string" },
        search: { type: "string" },
        filters: { type: "object", additionalProperties: { type: "string" } },
        columns: { type: "array", items: { type: "string" } },
        limit: { type: "integer", minimum: 1 },
      },
      required: ["app", "dataset_id"],
    },
  },
  {
    name: "mei_query_metric",
    description: "Run bounded runtime metric queries for a dataset.",
    inputSchema: {
      type: "object",
      properties: {
        app: { type: "string" },
        source_root: { type: "string" },
        dataset_id: { type: "string" },
        metric_ids: { type: "array", items: { type: "string" } },
        scene: { type: "string" },
        target_file: { type: "string" },
        search: { type: "string" },
        filters: { type: "object", additionalProperties: { type: "string" } },
      },
      required: ["app", "dataset_id"],
    },
  },
  {
    name: "mei_query_resource",
    description: "Fetch a single world resource/entity payload.",
    inputSchema: {
      type: "object",
      properties: {
        app: { type: "string" },
        source_root: { type: "string" },
        resource_id: { type: "string" },
        scene: { type: "string" },
        target_file: { type: "string" },
      },
      required: ["app", "resource_id"],
    },
  },
  {
    name: "mei_runtime_peek",
    description: "Peek current runtime phase/result/actions for the selected scope.",
    inputSchema: {
      type: "object",
      properties: {
        app: { type: "string" },
        source_root: { type: "string" },
        scene: { type: "string" },
        target_file: { type: "string" },
        trace_limit: { type: "integer", minimum: 1 },
      },
      required: ["app"],
    },
  },
];

function writeMessage(payload) {
  const body = Buffer.from(JSON.stringify(payload), "utf8");
  const header = Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8");
  process.stdout.write(Buffer.concat([header, body]));
}

function reply(id, result) {
  writeMessage({ jsonrpc: "2.0", id, result });
}

function replyError(id, code, message) {
  writeMessage({
    jsonrpc: "2.0",
    id,
    error: { code, message },
  });
}

function nonEmptyString(value, name) {
  const text = typeof value === "string" ? value.trim() : "";
  if (!text) {
    throw new Error(`\`${name}\` is required`);
  }
  return text;
}

function optionalString(value) {
  if (typeof value !== "string") {
    return "";
  }
  return value.trim();
}

function buildScopeArgs(args = {}) {
  const cli = [];
  const sourceRoot = optionalString(args.source_root) || DEFAULT_SOURCE_ROOT;
  if (sourceRoot) {
    cli.push("--source-root", sourceRoot);
  }
  const scene = optionalString(args.scene);
  if (scene) {
    cli.push("--scene", scene);
  }
  const targetFile = optionalString(args.target_file);
  if (targetFile) {
    cli.push("--target-file", targetFile);
  }
  return cli;
}

function appendFilters(cli, filters) {
  if (!filters || typeof filters !== "object" || Array.isArray(filters)) {
    return;
  }
  for (const [key, raw] of Object.entries(filters)) {
    if (!key || raw == null) continue;
    const value = String(raw).trim();
    if (!value) continue;
    cli.push("--filter", `${key}=${value}`);
  }
}

function appendStringList(cli, flag, values) {
  if (!Array.isArray(values)) return;
  for (const item of values) {
    const text = optionalString(item);
    if (!text) continue;
    cli.push(flag, text);
  }
}

function buildToolCommand(name, args = {}) {
  const cli = [];
  switch (name) {
    case "mei_check": {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args);
      cli.push("check", "--app", app, ...scopeArgs, "--json");
      return cli;
    }
    case "mei_compile": {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args);
      cli.push("compile", "--app", app, ...scopeArgs, "--json");
      return cli;
    }
    case "mei_inspect_world": {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args);
      cli.push("inspect", "world", "--app", app, ...scopeArgs, "--json");
      return cli;
    }
    case "mei_inspect_inventory": {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args);
      cli.push("inspect", "inventory", "--app", app, ...scopeArgs, "--json");
      return cli;
    }
    case "mei_query_dataset": {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args);
      const datasetId = nonEmptyString(args.dataset_id, "dataset_id");
      cli.push("query", "dataset", "--app", app, "--id", datasetId, ...scopeArgs);
      const search = optionalString(args.search);
      if (search) {
        cli.push("--search", search);
      }
      appendFilters(cli, args.filters);
      appendStringList(cli, "--column", args.columns);
      if (Number.isInteger(args.limit) && args.limit > 0) {
        cli.push("--limit", String(args.limit));
      }
      cli.push("--json");
      return cli;
    }
    case "mei_query_metric": {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args);
      const datasetId = nonEmptyString(args.dataset_id, "dataset_id");
      cli.push("query", "metric", "--app", app, "--id", datasetId, ...scopeArgs);
      appendStringList(cli, "--metric-id", args.metric_ids);
      const search = optionalString(args.search);
      if (search) {
        cli.push("--search", search);
      }
      appendFilters(cli, args.filters);
      cli.push("--json");
      return cli;
    }
    case "mei_query_resource": {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args);
      const resourceId = nonEmptyString(args.resource_id, "resource_id");
      cli.push("query", "resource", "--app", app, "--id", resourceId, ...scopeArgs, "--json");
      return cli;
    }
    case "mei_runtime_peek": {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args);
      cli.push("runtime", "peek", "--app", app, ...scopeArgs);
      if (Number.isInteger(args.trace_limit) && args.trace_limit > 0) {
        cli.push("--trace-limit", String(args.trace_limit));
      }
      cli.push("--json");
      return cli;
    }
    case "mei_host_describe":
      cli.push("host", "describe", "--json");
      return cli;
    default:
      throw new Error(`unknown tool: ${name}`);
  }
}

function runMei(cliArgs) {
  return new Promise((resolve, reject) => {
    const child = spawn(MEI_BIN, cliArgs, {
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
    });
    let stdout = "";
    let stderr = "";

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", (error) => {
      reject(error);
    });
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(stderr.trim() || `mei exited with code ${code}`));
        return;
      }
      const text = stdout.trim();
      if (!text) {
        resolve({ raw: "" });
        return;
      }
      try {
        resolve(JSON.parse(text));
      } catch {
        resolve({ raw: text });
      }
    });
  });
}

async function handleToolCall(id, params = {}) {
  const name = optionalString(params.name);
  const args = params.arguments && typeof params.arguments === "object" ? params.arguments : {};
  if (!name) {
    replyError(id, -32602, "tools/call requires `name`");
    return;
  }
  if (!TOOL_DEFS.some((tool) => tool.name === name)) {
    replyError(id, -32602, `unsupported tool: ${name}`);
    return;
  }
  try {
    const cliArgs = buildToolCommand(name, args);
    const result = await runMei(cliArgs);
    reply(id, {
      content: [{ type: "text", text: JSON.stringify(result, null, 2) }],
      structuredContent: result,
    });
  } catch (error) {
    reply(id, {
      isError: true,
      content: [
        {
          type: "text",
          text: String(error instanceof Error ? error.message : error),
        },
      ],
    });
  }
}

async function handleMessage(msg) {
  if (!msg || msg.jsonrpc !== "2.0") {
    return;
  }
  const { id, method, params } = msg;
  if (method === "notifications/initialized") {
    return;
  }
  if (method === "initialize") {
    reply(id, {
      protocolVersion: "2024-11-05",
      capabilities: { tools: {} },
      serverInfo: {
        name: "mei-editor-stdio-adapter",
        version: "0.1.0",
      },
    });
    return;
  }
  if (method === "tools/list") {
    reply(id, {
      tools: TOOL_DEFS.map((tool) => ({
        name: tool.name,
        description: tool.description,
        inputSchema: tool.inputSchema,
      })),
    });
    return;
  }
  if (method === "tools/call") {
    await handleToolCall(id, params);
    return;
  }
  if (id !== undefined) {
    replyError(id, -32601, `method not found: ${method}`);
  }
}

let buffer = Buffer.alloc(0);
process.stdin.on("data", async (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (true) {
    const marker = buffer.indexOf("\r\n\r\n");
    if (marker === -1) {
      break;
    }
    const header = buffer.slice(0, marker).toString("utf8");
    const match = header.match(/Content-Length:\s*(\d+)/i);
    if (!match) {
      buffer = Buffer.alloc(0);
      break;
    }
    const bodyLength = Number(match[1]);
    const frameLength = marker + 4 + bodyLength;
    if (buffer.length < frameLength) {
      break;
    }
    const body = buffer.slice(marker + 4, frameLength).toString("utf8");
    buffer = buffer.slice(frameLength);
    let msg;
    try {
      msg = JSON.parse(body);
    } catch {
      continue;
    }
    try {
      await handleMessage(msg);
    } catch (error) {
      if (msg?.id !== undefined) {
        replyError(
          msg.id,
          -32603,
          String(error instanceof Error ? error.message : error),
        );
      }
    }
  }
});
