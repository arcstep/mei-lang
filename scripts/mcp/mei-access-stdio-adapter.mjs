#!/usr/bin/env node

import {
  appendFilters,
  appendStringList,
  buildScopeArgs,
  catalogToolToMcpTool,
  loadSurfaceDescriptor,
  nonEmptyString,
  optionalString,
  runMei,
  toolchainBinCandidates,
} from "./mcp-adapter-common.mjs";

const TOOLCHAIN_BIN_CANDIDATES = toolchainBinCandidates();
const DEFAULT_SOURCE_ROOT = process.env.MEI_SOURCE_ROOT || "";

let surfaceLoadPromise = null;
let cachedTools = null;
let cachedToolNames = null;

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

async function ensureAccessToolsLoaded() {
  if (cachedTools) {
    return cachedTools;
  }
  if (!surfaceLoadPromise) {
    surfaceLoadPromise = loadSurfaceDescriptor("access", TOOLCHAIN_BIN_CANDIDATES).then(
      ({ descriptor }) => {
        cachedTools = descriptor.tools.map(catalogToolToMcpTool);
        cachedToolNames = new Set(cachedTools.map((tool) => tool.name));
        const unsupported = cachedTools
          .map((tool) => tool.name)
          .filter((name) => !ACCESS_TOOL_COMMAND_BUILDERS.has(name));
        if (unsupported.length > 0) {
          throw new Error(
            `catalog exposes unsupported access tools without CLI mapping: ${unsupported.join(", ")}`,
          );
        }
        return cachedTools;
      },
    );
  }
  return surfaceLoadPromise;
}

const ACCESS_TOOL_COMMAND_BUILDERS = new Map([
  [
    "dataset_query",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      const datasetId = nonEmptyString(args.dataset_id, "dataset_id");
      const cli = ["query", "dataset", "--app", app, "--id", datasetId, ...scopeArgs];
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
      return { binCandidates: TOOLCHAIN_BIN_CANDIDATES, cli };
    },
  ],
  [
    "dataset_metric",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      const datasetId = nonEmptyString(args.dataset_id, "dataset_id");
      const cli = ["query", "metric", "--app", app, "--id", datasetId, ...scopeArgs];
      appendStringList(cli, "--metric-id", args.metric_ids);
      const search = optionalString(args.search);
      if (search) {
        cli.push("--search", search);
      }
      appendFilters(cli, args.filters);
      cli.push("--json");
      return { binCandidates: TOOLCHAIN_BIN_CANDIDATES, cli };
    },
  ],
  [
    "resource_list",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      return {
        binCandidates: TOOLCHAIN_BIN_CANDIDATES,
        cli: ["inspect", "inventory", "--app", app, ...scopeArgs, "--json"],
      };
    },
  ],
  [
    "resource_get",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      const resourceId = nonEmptyString(args.resource_id, "resource_id");
      return {
        binCandidates: TOOLCHAIN_BIN_CANDIDATES,
        cli: [
          "query",
          "resource",
          "--app",
          app,
          "--id",
          resourceId,
          ...scopeArgs,
          "--json",
        ],
      };
    },
  ],
  [
    "resource_runtime_peek",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      const cli = ["runtime", "peek", "--app", app, ...scopeArgs];
      if (Number.isInteger(args.trace_limit) && args.trace_limit > 0) {
        cli.push("--trace-limit", String(args.trace_limit));
      }
      cli.push("--json");
      return { binCandidates: TOOLCHAIN_BIN_CANDIDATES, cli };
    },
  ],
  [
    "resource_runtime_trace_export",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      const cli = ["export", "runtime-trace", "--app", app, ...scopeArgs];
      if (Number.isInteger(args.trace_limit) && args.trace_limit > 0) {
        cli.push("--trace-limit", String(args.trace_limit));
      }
      cli.push("--json");
      return { binCandidates: TOOLCHAIN_BIN_CANDIDATES, cli };
    },
  ],
  [
    "resource_business_summary",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      return {
        binCandidates: TOOLCHAIN_BIN_CANDIDATES,
        cli: ["inspect", "summary", "--app", app, ...scopeArgs, "--json"],
      };
    },
  ],
]);

function buildToolCommand(name, args = {}) {
  const builder = ACCESS_TOOL_COMMAND_BUILDERS.get(name);
  if (!builder) {
    throw new Error(`unknown tool: ${name}`);
  }
  return builder(args);
}

async function handleToolCall(id, params = {}) {
  const name = optionalString(params.name);
  const args = params.arguments && typeof params.arguments === "object" ? params.arguments : {};
  if (!name) {
    replyError(id, -32602, "tools/call requires `name`");
    return;
  }
  await ensureAccessToolsLoaded();
  if (!cachedToolNames.has(name)) {
    replyError(id, -32602, `unsupported tool: ${name}`);
    return;
  }
  try {
    const { binCandidates, cli } = buildToolCommand(name, args);
    const result = await runMei(binCandidates, cli);
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
        name: "mei-access-stdio-adapter",
        version: "0.1.0",
      },
    });
    return;
  }
  if (method === "tools/list") {
    const tools = await ensureAccessToolsLoaded();
    reply(id, { tools });
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
