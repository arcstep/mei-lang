#!/usr/bin/env node
/**
 * Route A acceptance: unified view surfaces should serve thin HTML documents (<= 500KB).
 * Usage: node scripts/thin-document-size-audit.mjs [baseUrl]
 */
const base = (process.argv[2] || "http://127.0.0.1:9527").replace(/\/+$/, "");
const MAX_BYTES = 512000;
const surfaces = ["app", "layout", "prototype"];

async function headDocument(surface) {
  const url = `${base}/apps/data-demo/view?surface=${encodeURIComponent(surface)}`;
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
  return { surface, url, ...html };
}

async function fetchMeta(surface) {
  const url = `${base}/apps/data-demo/view?surface=${encodeURIComponent(surface)}`;
  const response = await fetch(url, { redirect: "follow" });
  const html = await response.text();
  const bootstrapInlined = /meta name="mei-bootstrap-inlined" content="0"/.test(html);
  const drilldownInlined = /meta name="mei-drilldown-inlined" content="0"/.test(html);
  const composePlaceholder = /id="mei-compose-root"[^>]*data-mei-compose-placeholder="1"/.test(html);
  return {
    surface,
    bootstrapInlined,
    drilldownInlined,
    composePlaceholder,
    htmlBytes: Buffer.byteLength(html, "utf8"),
  };
}

async function main() {
  const failures = [];
  const headResults = [];
  for (const surface of surfaces) {
    const result = await headDocument(surface);
    headResults.push(result);
    if (result.status !== 200) {
      failures.push(`${surface}: expected HTTP 200, got ${result.status}`);
    }
    if (result.bytes > MAX_BYTES) {
      failures.push(
        `${surface}: content-length ${result.bytes} exceeds ${MAX_BYTES} bytes`,
      );
    }
  }

  const metaResults = [];
  for (const surface of surfaces) {
    const meta = await fetchMeta(surface);
    metaResults.push(meta);
    if (!meta.bootstrapInlined) {
      failures.push(`${surface}: expected mei-bootstrap-inlined=0`);
    }
    if (!meta.drilldownInlined) {
      failures.push(`${surface}: expected mei-drilldown-inlined=0`);
    }
    if (surface === "app" && !meta.composePlaceholder) {
      failures.push(`${surface}: expected data-mei-compose-placeholder=1 on compose root`);
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
