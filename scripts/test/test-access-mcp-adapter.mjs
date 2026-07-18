#!/usr/bin/env node

import { spawn } from "node:child_process";
import assert from "node:assert/strict";
import { loadSurfaceDescriptor, toolchainBinCandidates } from "../mcp/mcp-adapter-common.mjs";

function encodeMessage(payload) {
  const body = Buffer.from(JSON.stringify(payload), "utf8");
  return Buffer.concat([Buffer.from(`Content-Length: ${body.length}\r\n\r\n`, "utf8"), body]);
}

function createFrameReader(onMessage) {
  let buffer = Buffer.alloc(0);
  return (chunk) => {
    buffer = Buffer.concat([buffer, chunk]);
    while (true) {
      const marker = buffer.indexOf("\r\n\r\n");
      if (marker === -1) return;
      const header = buffer.slice(0, marker).toString("utf8");
      const match = header.match(/Content-Length:\s*(\d+)/i);
      if (!match) {
        buffer = Buffer.alloc(0);
        return;
      }
      const length = Number(match[1]);
      const frameEnd = marker + 4 + length;
      if (buffer.length < frameEnd) return;
      const body = buffer.slice(marker + 4, frameEnd).toString("utf8");
      buffer = buffer.slice(frameEnd);
      try {
        onMessage(JSON.parse(body));
      } catch {
        // Ignore malformed frames in smoke test.
      }
    }
  };
}

function waitForResponse(queue, id, timeoutMs = 10000) {
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const tick = () => {
      for (let i = 0; i < queue.length; i += 1) {
        if (queue[i] && queue[i].id === id) {
          const msg = queue.splice(i, 1)[0];
          resolve(msg);
          return;
        }
      }
      if (Date.now() - start > timeoutMs) {
        reject(new Error(`timeout waiting for response id=${id}`));
        return;
      }
      setTimeout(tick, 30);
    };
    tick();
  });
}

async function main() {
  const binCandidates = toolchainBinCandidates();
  const { descriptor: catalogSurface } = await loadSurfaceDescriptor("access", binCandidates);
  const catalogNames = catalogSurface.tools.map((tool) => tool.name).sort();

  const child = spawn(process.execPath, ["./scripts/mcp/mei-access-stdio-adapter.mjs"], {
    cwd: process.cwd(),
    stdio: ["pipe", "pipe", "pipe"],
    env: process.env,
  });

  const queue = [];
  const onStdout = createFrameReader((msg) => queue.push(msg));
  child.stdout.on("data", onStdout);

  let stderr = "";
  child.stderr.on("data", (chunk) => {
    stderr += chunk.toString("utf8");
  });

  const send = (payload) => {
    child.stdin.write(encodeMessage(payload));
  };

  try {
    send({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: {
        protocolVersion: "2024-11-05",
        capabilities: {},
        clientInfo: { name: "mei-smoke", version: "0.1.0" },
      },
    });
    const init = await waitForResponse(queue, 1);
    assert.equal(init.result?.serverInfo?.name, "mei-access-stdio-adapter");

    send({ jsonrpc: "2.0", method: "notifications/initialized", params: {} });

    send({ jsonrpc: "2.0", id: 2, method: "tools/list", params: {} });
    const tools = await waitForResponse(queue, 2);
    const names = (tools.result?.tools || []).map((tool) => tool.name).sort();
    assert.deepEqual(
      names,
      catalogNames,
      "tools/list should match mei-toolchain mcp describe --surface access",
    );
    assert.ok(names.includes("dataset_query"));
    assert.ok(names.includes("resource_business_summary"));
  } finally {
    child.kill("SIGTERM");
    await new Promise((resolve) => {
      child.once("close", resolve);
      setTimeout(resolve, 1000);
    });
  }

  if (stderr.trim()) {
    process.stderr.write(stderr);
  }
  process.stdout.write("access MCP adapter smoke passed\n");
}

main().catch((error) => {
  process.stderr.write(String(error instanceof Error ? error.stack || error.message : error));
  process.stderr.write("\n");
  process.exit(1);
});
