#!/usr/bin/env node
/**
 * Route A acceptance: unified view surfaces should serve thin HTML documents (<= 32KB).
 * Usage: node scripts/audit/thin-document-size-audit.mjs [baseUrl]
 */
import { resolveAppId } from "../lib/resolve-app.mjs";

const appId = resolveAppId();
const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const MAX_BYTES = 32 * 1024;
const routes = [
  { name: `${appId}/home`, url: `${base}/apps/${appId}/home` },
];

async function headDocument(route) {
  const { name, url } = route;
  const response = await fetch(url, { method: "HEAD", redirect: "follow" });
  const lengthHeader = response.headers.get("content-length");
  const bytes = lengthHeader ? Number(lengthHeader) : null;
  const html =
    bytes == null
      ? await (async () => {
          const getResponse = await fetch(url, { redirect: "follow" });
          const text = await getResponse.text();
          return { bytes: Buffer.byteLength(text, "utf8"), status: getResponse.status };
        })()
      : { bytes, status: response.status };
  return { route: name, url, ...html };
}

async function fetchMeta(route) {
  const { name, url } = route;
  const response = await fetch(url, { redirect: "follow" });
  const html = await response.text();
  const bootstrapPayloadInlined =
    /meta name="mei-bootstrap-inlined" content="1"/.test(html) ||
    /id="mei-client-bootstrap"/.test(html);
  const drilldownInlined = /meta name="mei-drilldown-inlined" content="0"/.test(html);
  const composePlaceholder = /id="mei-compose-root"[^>]*data-mei-compose-placeholder="1"/.test(html);
  const refsStart = html.indexOf("window.__mei.scene_manifest_refs=");
  const refsEnd =
    refsStart >= 0 ? html.indexOf(";window.__mei.thin_shell", refsStart) : -1;
  const manifestRefsText =
    refsStart >= 0 && refsEnd > refsStart ? html.slice(refsStart, refsEnd) : "";
  return {
    route: name,
    bootstrapPayloadInlined,
    drilldownInlined,
    composePlaceholder,
    revisionEnvelope: html.includes("window.__mei.view_revision_envelope="),
    manifestRefsHasLayers: /"layers"\s*:/.test(manifestRefsText),
    htmlBytes: Buffer.byteLength(html, "utf8"),
  };
}

async function main() {
  const failures = [];
  const headResults = [];
  for (const route of routes) {
    const result = await headDocument(route);
    headResults.push(result);
    if (result.status !== 200) {
      failures.push(`${route.name}: expected HTTP 200, got ${result.status}`);
    }
    if (result.bytes > MAX_BYTES) {
      failures.push(
        `${route.name}: content-length ${result.bytes} exceeds ${MAX_BYTES} bytes`,
      );
    }
  }

  const metaResults = [];
  for (const route of routes) {
    const meta = await fetchMeta(route);
    metaResults.push(meta);
    if (meta.bootstrapPayloadInlined) {
      failures.push(`${route.name}: thin document must not inline eval bootstrap`);
    }
    if (!meta.drilldownInlined) {
      failures.push(`${route.name}: expected mei-drilldown-inlined=0`);
    }
    if (!meta.revisionEnvelope) {
      failures.push(`${route.name}: expected view_revision_envelope`);
    }
    if (meta.manifestRefsHasLayers) {
      failures.push(`${route.name}: thin document must not contain full layer manifest`);
    }
    if (!meta.composePlaceholder) {
      failures.push(`${route.name}: expected data-mei-compose-placeholder=1 on compose root`);
    }
  }

  const report = {
    ok: failures.length === 0,
    base,
    maxBytes: MAX_BYTES,
    headResults,
    metaResults,
    failures,
  };
  console.log(JSON.stringify(report, null, 2));
  if (failures.length > 0) {
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
