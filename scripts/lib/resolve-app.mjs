#!/usr/bin/env node
/**
 * Resolve live-host app id for platform audit/perf scripts.
 * No business defaults (zhifa/qunfu/…) in the public mei-lang package.
 *
 * Sources (first wins): --app / --app=…, then MEI_APP_ID, then APP_ID.
 */
export function resolveAppId(options = {}) {
  const argv = options.argv ?? process.argv.slice(2);
  let fromFlag = "";
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--app" && argv[i + 1] && !String(argv[i + 1]).startsWith("-")) {
      fromFlag = String(argv[i + 1]).trim();
      break;
    }
    if (typeof arg === "string" && arg.startsWith("--app=")) {
      fromFlag = arg.slice("--app=".length).trim();
      break;
    }
  }
  const appId = String(fromFlag || process.env.MEI_APP_ID || process.env.APP_ID || "").trim();
  if (!appId) {
    console.error(
      "error: set MEI_APP_ID (or APP_ID) or pass --app <id>; mei-lang has no business app default",
    );
    console.error(
      "hint: from a workspace profile scripts/audit wrapper that exports MEI_APP_ID",
    );
    process.exit(2);
  }
  return appId;
}

export function resolveBaseUrl(options = {}) {
  const argv = options.argv ?? process.argv.slice(2);
  const positional = argv.find((a) => typeof a === "string" && !a.startsWith("-"));
  const raw =
    options.envBase ??
    process.env.MEI_E2E_BASE_URL ??
    process.env.MEI_SERVER_URL ??
    positional ??
    "http://127.0.0.1:9527";
  return String(raw).trim().replace(/\/+$/, "");
}
