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
  uniqueNonEmpty,
} from "./mcp-adapter-common.mjs";

const TOOLCHAIN_BIN_CANDIDATES = toolchainBinCandidates();
const HOST_WEB_BIN_CANDIDATES = uniqueNonEmpty([
  process.env.MEI_HOST_WEB_BIN,
  process.env.MEI_HOST_BIN,
  process.env.MEI_BIN,
  "mei-host-web",
  "mei",
]);
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

async function ensureAuthorToolsLoaded() {
  if (cachedTools) {
    return cachedTools;
  }
  if (!surfaceLoadPromise) {
    surfaceLoadPromise = loadSurfaceDescriptor("author", TOOLCHAIN_BIN_CANDIDATES).then(
      ({ descriptor }) => {
        cachedTools = descriptor.tools.map(catalogToolToMcpTool);
        cachedToolNames = new Set(cachedTools.map((tool) => tool.name));
        const unsupported = cachedTools
          .map((tool) => tool.name)
          .filter((name) => !AUTHOR_TOOL_COMMAND_BUILDERS.has(name));
        if (unsupported.length > 0) {
          throw new Error(
            `catalog exposes unsupported author tools without CLI mapping: ${unsupported.join(", ")}`,
          );
        }
        return cachedTools;
      },
    );
  }
  return surfaceLoadPromise;
}

const AUTHOR_TOOL_COMMAND_BUILDERS = new Map([
  [
    "mei_author_knowledge",
    (args) => {
      const cli = ["knowledge", "--surface", "editor"];
      const topic = optionalString(args.topic);
      if (topic) {
        cli.push("--topic", topic);
      }
      if (args.include_content === true) {
        cli.push("--include-content");
      }
      cli.push("--json");
      return {
        binCandidates: TOOLCHAIN_BIN_CANDIDATES,
        cli,
      };
    },
  ],
  [
    "mei_editor_runtime_describe",
    () => ({
      binCandidates: TOOLCHAIN_BIN_CANDIDATES,
      cli: ["editor-runtime", "describe", "--json"],
    }),
  ],
  [
    "mei_editor_runtime_doctor",
    () => ({
      binCandidates: TOOLCHAIN_BIN_CANDIDATES,
      cli: ["editor-runtime", "doctor", "--json"],
    }),
  ],
  [
    "mei_check",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      return {
        binCandidates: TOOLCHAIN_BIN_CANDIDATES,
        cli: ["check", "--app", app, ...scopeArgs, "--json"],
      };
    },
  ],
  [
    "mei_compile",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      return {
        binCandidates: TOOLCHAIN_BIN_CANDIDATES,
        cli: ["compile", "--app", app, ...scopeArgs, "--json"],
      };
    },
  ],
  [
    "mei_workspace_summary",
    (args) => ({
      binCandidates: TOOLCHAIN_BIN_CANDIDATES,
      cli: ["workspace", "summary", ...buildScopeArgs(args, DEFAULT_SOURCE_ROOT), "--json"],
    }),
  ],
  [
    "mei_inspect_world",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      return {
        binCandidates: TOOLCHAIN_BIN_CANDIDATES,
        cli: ["inspect", "world", "--app", app, ...scopeArgs, "--json"],
      };
    },
  ],
  [
    "mei_inspect_inventory",
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
    "mei_inspect_summary",
    (args) => {
      const app = nonEmptyString(args.app, "app");
      const scopeArgs = buildScopeArgs(args, DEFAULT_SOURCE_ROOT);
      return {
        binCandidates: TOOLCHAIN_BIN_CANDIDATES,
        cli: ["inspect", "summary", "--app", app, ...scopeArgs, "--json"],
      };
    },
  ],
  [
    "mei_query_dataset",
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
    "mei_query_metric",
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
    "mei_query_resource",
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
    "mei_runtime_peek",
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
    "mei_host_describe",
    () => ({
      binCandidates: HOST_WEB_BIN_CANDIDATES,
      cli: ["host", "describe", "--json"],
    }),
  ],
]);

function buildToolCommand(name, args = {}) {
  const builder = AUTHOR_TOOL_COMMAND_BUILDERS.get(name);
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
  await ensureAuthorToolsLoaded();
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
        name: "mei-editor-stdio-adapter",
        version: "0.2.0",
      },
    });
    return;
  }
  if (method === "tools/list") {
    const tools = await ensureAuthorToolsLoaded();
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
