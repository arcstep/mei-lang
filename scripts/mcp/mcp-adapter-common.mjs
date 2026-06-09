#!/usr/bin/env node

import { spawn } from "node:child_process";

export function uniqueNonEmpty(values) {
  return [...new Set(values.filter((item) => typeof item === "string" && item.trim()))];
}

export function optionalString(value) {
  if (typeof value !== "string") {
    return "";
  }
  return value.trim();
}

export function nonEmptyString(value, name) {
  const text = optionalString(value);
  if (!text) {
    throw new Error(`\`${name}\` is required`);
  }
  return text;
}

export function catalogToolToMcpTool(tool) {
  return {
    name: tool.name,
    description: tool.description,
    inputSchema: tool.input_schema || tool.inputSchema || {
      type: "object",
      properties: {},
    },
  };
}

export function toolchainBinCandidates() {
  return uniqueNonEmpty([
    process.env.MEI_TOOLCHAIN_BIN,
    "./target/debug/mei-toolchain",
    "mei-toolchain",
  ]);
}

export async function loadSurfaceDescriptor(surface, binCandidates, sourceRoot = "") {
  const missingBins = [];
  for (const bin of binCandidates) {
    try {
      const cli = [
        "mcp",
        "describe",
        "--surface",
        surface,
      ];
      if (optionalString(sourceRoot)) {
        cli.push("--source-root", optionalString(sourceRoot));
      }
      cli.push("--json");
      const descriptor = await runSingleMei(bin, cli);
      if (!descriptor || typeof descriptor !== "object") {
        throw new Error(`invalid MCP surface descriptor from ${bin}`);
      }
      if (!Array.isArray(descriptor.tools)) {
        throw new Error(`MCP surface descriptor from ${bin} missing tools[]`);
      }
      return { descriptor, bin };
    } catch (error) {
      if (error && typeof error === "object" && error.code === "ENOENT") {
        missingBins.push(bin);
        continue;
      }
      throw error;
    }
  }
  throw new Error(
    `failed to load MCP surface '${surface}'; tried ${missingBins.join(", ")}`,
  );
}

function runSingleMei(bin, cliArgs) {
  return new Promise((resolve, reject) => {
    const child = spawn(bin, cliArgs, {
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
    });
    let stdout = "";
    let stderr = "";
    let spawnError = null;

    child.stdout.on("data", (chunk) => {
      stdout += chunk.toString("utf8");
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk.toString("utf8");
    });
    child.on("error", (error) => {
      spawnError = error;
    });
    child.on("close", (code) => {
      if (spawnError) {
        reject(spawnError);
        return;
      }
      if (code !== 0) {
        reject(new Error(stderr.trim() || `${bin} exited with code ${code}`));
        return;
      }
      const text = stdout.trim();
      if (!text) {
        resolve({});
        return;
      }
      try {
        resolve(JSON.parse(text));
      } catch (error) {
        reject(
          new Error(
            `${bin} returned non-JSON output: ${String(error instanceof Error ? error.message : error)}`,
          ),
        );
      }
    });
  });
}

export async function runMei(binCandidates, cliArgs) {
  const missingBins = [];
  for (const bin of binCandidates) {
    try {
      return await runSingleMei(bin, cliArgs);
    } catch (error) {
      if (error && typeof error === "object" && error.code === "ENOENT") {
        missingBins.push(bin);
        continue;
      }
      throw error;
    }
  }
  throw new Error(`no usable Mei CLI found; tried ${missingBins.join(", ")}`);
}

export function buildScopeArgs(args = {}, defaultSourceRoot = "") {
  const cli = [];
  const sourceRoot = optionalString(args.source_root) || defaultSourceRoot;
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

export function appendFilters(cli, filters) {
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

export function appendStringList(cli, flag, values) {
  if (!Array.isArray(values)) return;
  for (const item of values) {
    const text = optionalString(item);
    if (!text) continue;
    cli.push(flag, text);
  }
}
