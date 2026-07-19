function parseProps(element) {
  const raw = element.getAttribute("data-props");
  if (!raw) return {};
  try {
    return JSON.parse(raw);
  } catch (_error) {
    return {};
  }
}

function element(tag, options = {}) {
  const node = document.createElement(tag);
  if (options.className) node.className = options.className;
  if (options.text != null) node.textContent = String(options.text);
  if (options.type) node.type = options.type;
  return node;
}

const adminResourceCache = new Map();

function currentAdminContext() {
  const match = window.location.pathname.match(
    /^\/admin\/apps\/([^/]+)\/([^/]+)\/([^/]+)\/?$/,
  );
  if (!match) return null;
  return {
    appId: decodeURIComponent(match[1]),
    resourceId: decodeURIComponent(match[2]),
    moduleId: decodeURIComponent(match[3]),
  };
}

function providerRefId(value) {
  if (typeof value === "string") return value.trim();
  if (!value || typeof value !== "object") return "";
  if (value.__ref === "provider_ref" || value.__call === "provider_ref") {
    return String(value.__args?.arg0 || value.__args?.id || "").trim();
  }
  return value.kind === "provider_ref" ? String(value.id || "").trim() : "";
}

async function requestJson(url, options = {}) {
  const response = await fetch(url, { credentials: "same-origin", ...options });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload.message || `Admin provider request failed: ${response.status}`);
  }
  return payload;
}

async function currentAdminResource() {
  const context = currentAdminContext();
  if (!context) throw new Error("Admin route context is unavailable");
  const cacheKey = `${context.appId}/${context.resourceId}/${context.moduleId}`;
  if (!adminResourceCache.has(cacheKey)) {
    adminResourceCache.set(
      cacheKey,
      requestJson(`/api/admin/resources?app_id=${encodeURIComponent(context.appId)}`).then(
        (catalog) => {
          const resource = (catalog.resources || []).find(
            (entry) =>
              entry.registryEntry?.resourceId === context.resourceId &&
              entry.registryEntry?.moduleId === context.moduleId,
          );
          if (!resource) throw new Error(`Admin resource not found: ${cacheKey}`);
          return { context, resource };
        },
      ),
    );
  }
  return adminResourceCache.get(cacheKey);
}

async function resolveProvider(reference) {
  const bindingId = providerRefId(reference);
  if (!bindingId) throw new Error("provider_ref is missing");
  const { context, resource } = await currentAdminResource();
  const bindings = resource.pageProgram?.provider_bindings || [];
  const binding = bindings.find((entry) => entry.bindingId === bindingId);
  if (!binding) throw new Error(`ProviderBinding not found: ${bindingId}`);
  return {
    route: context,
    binding,
    context: {
      ...context,
      providerId: binding.providerId,
      method: binding.method,
      target: binding.target,
    },
  };
}

function providerEndpoint(route, providerId, suffix = "") {
  return `/api/admin/apps/${encodeURIComponent(route.appId)}/${encodeURIComponent(
    route.resourceId,
  )}/${encodeURIComponent(route.moduleId)}/providers/${providerId}${suffix}`;
}

async function readProvider(reference, extra = {}) {
  const resolved = await resolveProvider(reference);
  const query = new URLSearchParams({ ...resolved.context, ...extra });
  return requestJson(
    `${providerEndpoint(resolved.route, resolved.binding.providerId)}?${query.toString()}`,
  );
}

function idempotencyKey() {
  return globalThis.crypto?.randomUUID?.() || `admin-${Date.now()}-${Math.random()}`;
}

async function putConfigRecord(reference, payload, revision) {
  const resolved = await resolveProvider(reference);
  return requestJson(providerEndpoint(resolved.route, resolved.binding.providerId), {
    method: "PUT",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      ...resolved.context,
      revision,
      idempotencyKey: idempotencyKey(),
      payload,
    }),
  });
}

async function invokeProviderAction(reference, payload = {}) {
  const resolved = await resolveProvider(reference);
  return requestJson(providerEndpoint(resolved.route, resolved.binding.providerId), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      ...resolved.context,
      idempotencyKey: idempotencyKey(),
      payload,
    }),
  });
}

async function replaceAsset(reference, file) {
  const resolved = await resolveProvider(reference);
  const bytes = new Uint8Array(await file.arrayBuffer());
  const contentHex = Array.from(bytes, (value) => value.toString(16).padStart(2, "0")).join("");
  return requestJson(providerEndpoint(resolved.route, resolved.binding.providerId, "/replace"), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      ...resolved.context,
      filename: file.name,
      idempotencyKey: idempotencyKey(),
      contentHex,
    }),
  });
}

async function applyAssetCurrent(reference, filename) {
  const resolved = await resolveProvider(reference);
  return requestJson(
    providerEndpoint(resolved.route, resolved.binding.providerId, "/apply-current"),
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        ...resolved.context,
        filename,
        idempotencyKey: idempotencyKey(),
      }),
    },
  );
}

async function deleteAssetFile(reference, filename) {
  const resolved = await resolveProvider(reference);
  return requestJson(
    providerEndpoint(resolved.route, resolved.binding.providerId, "/delete-file"),
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        ...resolved.context,
        filename,
        idempotencyKey: idempotencyKey(),
      }),
    },
  );
}

async function downloadAssetFile(reference, filename) {
  const resolved = await resolveProvider(reference);
  const response = await fetch(
    providerEndpoint(resolved.route, resolved.binding.providerId, "/download-file"),
    {
      method: "POST",
      credentials: "same-origin",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        ...resolved.context,
        filename,
      }),
    },
  );
  if (!response.ok) {
    const payload = await response.json().catch(() => ({}));
    throw new Error(payload.message || `下载失败: ${response.status}`);
  }
  const blob = await response.blob();
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

function statusLabel(status) {
  const value = String(status || "").toLowerCase();
  if (value === "ready") return "就绪";
  if (value === "pending") return "待配置";
  if (value === "missing") return "缺失";
  return status || "—";
}

function truncatePath(value, max = 48) {
  const text = String(value || "");
  if (text.length <= max) return text;
  const head = Math.max(12, Math.floor(max * 0.35));
  const tail = max - head - 1;
  return `${text.slice(0, head)}…${text.slice(-tail)}`;
}

function formatBytes(value) {
  const n = Number(value);
  if (!Number.isFinite(n) || n < 0) return "—";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function parseExplorerSort(raw, fallbackField = "name") {
  const text = String(raw || "").trim();
  const match = text.match(/^([a-zA-Z_]+):(asc|desc)$/i);
  if (!match) return { field: fallbackField, dir: "asc" };
  return { field: match[1].toLowerCase(), dir: match[2].toLowerCase() };
}

function formatExplorerSort(sort) {
  return `${sort.field}:${sort.dir}`;
}

function buildExplorerSortSelect(options) {
  const {
    fields = ["name"],
    value,
    labels = {},
    ariaLabel = "排序",
    onChange,
  } = options || {};
  const select = element("select", { className: "mei-admin-explorer-sort" });
  select.setAttribute("aria-label", ariaLabel);
  const fieldLabels = {
    name: "名称",
    title: "标题",
    size: "大小",
    time: "时间",
    ...labels,
  };
  fields.forEach((field) => {
    ["asc", "desc"].forEach((dir) => {
      const option = document.createElement("option");
      option.value = `${field}:${dir}`;
      option.textContent = `${fieldLabels[field] || field}${dir === "asc" ? "升序" : "降序"}`;
      select.append(option);
    });
  });
  select.value = formatExplorerSort(value);
  if (![...select.options].some((opt) => opt.value === select.value) && select.options.length) {
    select.value = select.options[0].value;
  }
  select.addEventListener("change", () => {
    onChange?.(parseExplorerSort(select.value, fields[0] || "name"));
  });
  return select;
}

function compareText(left, right) {
  return String(left || "").localeCompare(String(right || ""), "zh-CN", {
    sensitivity: "base",
    numeric: true,
  });
}

function compareNumber(left, right) {
  const a = Number(left);
  const b = Number(right);
  const aOk = Number.isFinite(a);
  const bOk = Number.isFinite(b);
  if (!aOk && !bOk) return 0;
  if (!aOk) return 1;
  if (!bOk) return -1;
  return a - b;
}

function sortShareEntries(entries, sort) {
  const dir = sort?.dir === "desc" ? -1 : 1;
  const field = sort?.field || "name";
  return [...entries].sort((left, right) => {
    const leftDir = Boolean(left?.isDir);
    const rightDir = Boolean(right?.isDir);
    if (leftDir !== rightDir) return leftDir ? -1 : 1;
    let cmp = 0;
    if (field === "size") {
      cmp = compareNumber(left?.sizeBytes ?? left?.size_bytes, right?.sizeBytes ?? right?.size_bytes);
      if (!cmp) cmp = compareText(left?.name, right?.name);
    } else if (field === "time") {
      cmp = compareNumber(
        left?.modifiedMs ?? left?.modified_ms,
        right?.modifiedMs ?? right?.modified_ms,
      );
      if (!cmp) cmp = compareText(left?.name, right?.name);
    } else {
      cmp = compareText(left?.name, right?.name);
    }
    return cmp * dir;
  });
}

function resourceSortSize(resource) {
  const slot = resource?.slot || {};
  return (
    resource?.sizeBytes ??
    resource?.size_bytes ??
    slot.sizeBytes ??
    slot.size_bytes ??
    null
  );
}

function resourceSortTime(resource) {
  const slot = resource?.slot || {};
  return (
    resource?.modifiedMs ??
    resource?.modified_ms ??
    slot.modifiedMs ??
    slot.modified_ms ??
    null
  );
}

function collectionSortFields(resources) {
  const fields = ["title"];
  if ((resources || []).some((resource) => resourceSortSize(resource) != null)) {
    fields.push("size");
  }
  if ((resources || []).some((resource) => resourceSortTime(resource) != null)) {
    fields.push("time");
  }
  return fields;
}

function sortCollectionResources(resources, sort) {
  const dir = sort?.dir === "desc" ? -1 : 1;
  const field = sort?.field || "title";
  return [...resources].sort((left, right) => {
    let cmp = 0;
    if (field === "size") {
      cmp = compareNumber(resourceSortSize(left), resourceSortSize(right));
      if (!cmp) cmp = compareText(left?.title || left?.id, right?.title || right?.id);
    } else if (field === "time") {
      cmp = compareNumber(resourceSortTime(left), resourceSortTime(right));
      if (!cmp) cmp = compareText(left?.title || left?.id, right?.title || right?.id);
    } else {
      cmp = compareText(left?.title || left?.id, right?.title || right?.id);
    }
    if (!cmp) {
      cmp = Number(Boolean(right?.recommended)) - Number(Boolean(left?.recommended));
    }
    return cmp * dir;
  });
}

function fileExtension(name) {
  const text = String(name || "");
  const index = text.lastIndexOf(".");
  if (index <= 0 || index === text.length - 1) return "";
  return text.slice(index + 1).toLowerCase();
}

function resolveFileKind(entryOrName, isDir = false) {
  if (isDir || (entryOrName && typeof entryOrName === "object" && entryOrName.isDir)) {
    return "folder";
  }
  const name = typeof entryOrName === "string" ? entryOrName : entryOrName?.name;
  const ext = fileExtension(name);
  const map = {
    xlsx: "xlsx",
    xls: "xls",
    csv: "csv",
    pdf: "pdf",
    doc: "doc",
    docx: "docx",
    ppt: "ppt",
    pptx: "pptx",
    jpg: "image",
    jpeg: "image",
    png: "image",
    gif: "image",
    webp: "image",
    svg: "image",
    mp4: "video",
    webm: "video",
    mov: "video",
    txt: "txt",
    json: "json",
    zip: "zip",
  };
  return map[ext] || "file";
}

function fileKindChipLabel(kind, name) {
  if (kind === "folder") return "文件夹";
  if (kind === "image" || kind === "video") {
    return fileExtension(name).toUpperCase() || (kind === "image" ? "图片" : "视频");
  }
  if (kind === "file") return "文件";
  return String(kind || "file").toUpperCase();
}

function fileKindIcon(kind) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "mei-admin-file-kind-icon");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  svg.dataset.fileKind = kind || "file";
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("fill", "currentColor");
  const glyphs = {
    folder:
      "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7z",
    xlsx:
      "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm7 1.5V8h3.5L13 4.5zM8.2 11l2.1 3.2L8.2 17.4h1.7l1.4-2.2 1.4 2.2h1.7L12.3 14.2 14.4 11h-1.7l-1.4 2.1L10 11H8.2z",
    xls: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm7 1.5V8h3.5L13 4.5zM8 12h2.2l1 1.8 1-1.8H14l-1.8 3L14 18h-1.8l-1-1.8-1 1.8H8l1.8-3L8 12z",
    csv: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm2 9h2v2H8v-2zm4 0h2v2h-2v-2zm4 0h2v2h-2v-2zm-8 3h2v2H8v-2zm4 0h2v2h-2v-2zm4 0h2v2h-2v-2z",
    pdf: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm7 1.5V8h3.5L13 4.5zM8 12h3.2a1.8 1.8 0 0 1 0 3.6H9.2V18H8v-6zm1.2 1.2v1.2h1.8a.6.6 0 1 0 0-1.2H9.2zM13 12h2.4c1.2 0 2 .8 2 2s-.8 2-2 2H14.2V18H13v-6zm1.2 1.2v1.6h1.2c.5 0 .8-.3.8-.8s-.3-.8-.8-.8h-1.2z",
    doc: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm2 9h8v1.2H8V12zm0 2.4h8V15.6H8v-1.2zm0 2.4h5.5V18H8v-1.2z",
    docx: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm2 9h8v1.2H8V12zm0 2.4h8V15.6H8v-1.2zm0 2.4h5.5V18H8v-1.2z",
    ppt: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm2 9h5a2.5 2.5 0 0 1 0 5H9.2V18H8v-6zm1.2 1.2v2.6H13a1.3 1.3 0 0 0 0-2.6H9.2z",
    pptx: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm2 9h5a2.5 2.5 0 0 1 0 5H9.2V18H8v-6zm1.2 1.2v2.6H13a1.3 1.3 0 0 0 0-2.6H9.2z",
    image:
      "M5 5h14a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zm2.5 3.5a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3zM6 17l3.8-4.2 2.4 2.6L15.2 12 18 17H6z",
    video:
      "M4 6h12a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1zm14.2 2.2 3.3-1.9a.6.6 0 0 1 .9.5v10.4a.6.6 0 0 1-.9.5l-3.3-1.9V8.2z",
    txt: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm2 9h8v1.2H8V12zm0 2.4h8V15.6H8v-1.2zm0 2.4h5V18H8v-1.2z",
    json: "M8 4c-1.7 0-3 1.3-3 3v2c0 .6-.4 1-1 1v4c.6 0 1 .4 1 1v2c0 1.7 1.3 3 3 3h1v-1.5H8c-.8 0-1.5-.7-1.5-1.5v-2c0-1.1-.7-2-1.7-2.3 1-.3 1.7-1.2 1.7-2.3V7c0-.8.7-1.5 1.5-1.5h1V4H8zm8 0h-1v1.5h1c.8 0 1.5.7 1.5 1.5v2c0 1.1.7 2 1.7 2.3-1 .3-1.7 1.2-1.7 2.3v2c0 .8-.7 1.5-1.5 1.5h-1V20h1c1.7 0 3-1.3 3-3v-2c0-.6.4-1 1-1v-4c-.6 0-1-.4-1-1V7c0-1.7-1.3-3-3-3z",
    zip: "M10 3h4v2h-1v2h1v2h-1v2h1v2h-1v2h1v2h-4V3zm1 2v2h2V5h-2zm0 4v2h2V9h-2zm0 4v2h2v-2h-2z",
    file: "M6 3h8l4 4v14a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zm7 1.5V8h3.5L13 4.5z",
  };
  path.setAttribute("d", glyphs[kind] || glyphs.file);
  svg.append(path);
  return svg;
}

function buildFileKindBadge(entryOrName, isDir = false) {
  const kind = resolveFileKind(entryOrName, isDir);
  const name = typeof entryOrName === "string" ? entryOrName : entryOrName?.name;
  const wrap = element("span", { className: "mei-admin-file-kind" });
  wrap.dataset.fileKind = kind;
  wrap.append(fileKindIcon(kind));
  const chip = element("span", {
    className: "mei-admin-chip mei-admin-file-kind-chip",
    text: fileKindChipLabel(kind, name),
  });
  wrap.append(chip);
  return wrap;
}

function updateExplorerUrl({ q, view, sel, sort }) {
  const url = new URL(window.location.href);
  const updates = { q, view, sel, sort };
  Object.entries(updates).forEach(([key, value]) => {
    if (value === undefined) return;
    const text = String(value || "").trim();
    if (text) url.searchParams.set(key, text);
    else url.searchParams.delete(key);
  });
  window.history.replaceState(window.history.state, "", url);
}

async function workspaceShareRequest(path, options = {}) {
  const response = await fetch(path, { credentials: "same-origin", ...options });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) {
    throw new Error(payload.message || payload.error || `资料交换请求失败: ${response.status}`);
  }
  return payload;
}

async function uploadWorkspaceShareFile(file, dir, onProgress) {
  const chunkSize = 4 * 1024 * 1024;
  if (file.size <= 8 * 1024 * 1024) {
    const form = new FormData();
    if (dir) form.append("dir", dir);
    form.append("idempotency_key", idempotencyKey());
    form.append("file", file, file.name);
    onProgress?.(0);
    const payload = await workspaceShareRequest("/api/workspace/share/upload", {
      method: "POST",
      body: form,
    });
    onProgress?.(1);
    return payload;
  }
  const init = await workspaceShareRequest("/api/workspace/share/chunk/init", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      file_name: file.name,
      dir: dir || null,
      size_bytes: file.size,
      chunk_size: chunkSize,
      last_modified_ms: file.lastModified || null,
      idempotency_key: idempotencyKey(),
    }),
  });
  const uploaded = new Set(init.uploadedChunks || []);
  for (let index = 0; index < init.totalChunks; index += 1) {
    if (!uploaded.has(index)) {
      const start = index * init.chunkSize;
      const body = file.slice(start, Math.min(start + init.chunkSize, file.size));
      await workspaceShareRequest(
        `/api/workspace/share/chunk?upload_id=${encodeURIComponent(
          init.uploadId,
        )}&index=${index}`,
        { method: "PUT", body },
      );
    }
    onProgress?.((index + 1) / init.totalChunks);
  }
  return workspaceShareRequest("/api/workspace/share/chunk/complete", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ upload_id: init.uploadId }),
  });
}

function downloadWorkspaceShareFile(path, revision) {
  const anchor = document.createElement("a");
  const query = new URLSearchParams({ path });
  if (revision) query.set("expected_revision", revision);
  anchor.href = `/api/workspace/share/download?${query.toString()}`;
  anchor.download = path.split("/").pop() || "download";
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
}

async function runCommandJob(reference, { assetBindingId, file }) {
  const resolved = await resolveProvider(reference);
  const bytes = new Uint8Array(await file.arrayBuffer());
  const contentHex = Array.from(bytes, (value) => value.toString(16).padStart(2, "0")).join("");
  return requestJson(providerEndpoint(resolved.route, resolved.binding.providerId), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      ...resolved.context,
      assetBindingId,
      filename: file.name,
      idempotencyKey: idempotencyKey(),
      contentHex,
    }),
  });
}

function ensureAdminStyles() {
  const styleId = "mei-admin-brick-styles-v8";
  document.getElementById("mei-admin-brick-styles")?.remove();
  document.getElementById("mei-admin-brick-styles-v3")?.remove();
  document.getElementById("mei-admin-brick-styles-v4")?.remove();
  document.getElementById("mei-admin-brick-styles-v5")?.remove();
  document.getElementById("mei-admin-brick-styles-v6")?.remove();
  document.getElementById("mei-admin-brick-styles-v7")?.remove();
  let style = document.getElementById(styleId);
  if (!style) {
    style = document.createElement("style");
    style.id = styleId;
    document.head.appendChild(style);
  }
  style.textContent = `
    .mei-compose-document-host {
      --mei-admin-chrome: 120px;
      --mei-admin-gutter: 14px;
      --mei-admin-form-min-w: 420px;
      --mei-admin-form-max-w: 640px;
      --mei-admin-form-pad: 20px 24px;
      --mei-admin-form-side-gap: clamp(16px, 4vw, 48px);
      box-sizing: border-box;
      height: calc(100dvh - var(--mei-admin-chrome));
      max-height: calc(100dvh - var(--mei-admin-chrome));
      min-height: 0;
      padding: var(--mei-admin-gutter);
      display: flex;
      flex-direction: column;
      overflow: hidden !important;
    }
    .mei-compose-document-host .preview-card {
      flex: 1; min-height: 0; height: 100%; padding: 0; border: 0; background: transparent;
      overflow: hidden; display: flex; flex-direction: column;
    }
    .mei-compose-document-host .component-host {
      flex: 1; min-height: 0; height: 100%; overflow: hidden; display: flex; flex-direction: column;
    }
    .mei-compose-document-host mei-admin-collection-view,
    .mei-compose-document-host mei-admin-form-card,
    .mei-compose-document-host mei-admin-grouped-form {
      flex: 1; min-height: 0; width: 100%; height: 100%; max-height: 100%;
      display: flex; flex-direction: column;
    }
    .mei-admin-entry-copy:empty { display: none; }
    .mei-admin-entry-copy { max-width: 1120px; line-height: 1.7; color: var(--mei-color-text-body, #cbd5e1); font-size: var(--mei-shell-font-1, 16px); }
    .mei-admin-entry-copy h2 { margin: 12px 0 6px; color: var(--mei-color-text-primary, #fff); font-size: var(--mei-shell-font-3, 18px); }
    .mei-admin-brick {
      max-width: 1120px; margin: 12px 0; padding: 20px;
      border: 1px solid var(--mei-color-border-default, rgba(148,163,184,.24)); border-radius: 10px;
      background: rgba(15, 23, 42, .72); box-sizing: border-box;
      font-family: inherit;
      font-size: var(--mei-shell-font-1, 16px);
      line-height: 1.45;
      color: var(--mei-color-text-body, #cbd5e1);
    }
    .mei-admin-form-page {
      max-width: none; width: 100%; flex: 1; min-height: 0; height: 100%;
      margin: 0; padding: var(--mei-admin-form-side-gap, clamp(16px, 4vw, 48px));
      border: 0; border-radius: 0; background: transparent;
      overflow: auto; overscroll-behavior: contain;
      display: flex; flex-direction: column; align-items: stretch;
    }
    .mei-admin-form-surface {
      --mei-admin-form-min-w: 420px;
      --mei-admin-form-max-w: 640px;
      --mei-admin-form-pad: 20px 24px;
      width: min(100%, var(--mei-admin-form-max-w));
      min-width: min(100%, var(--mei-admin-form-min-w));
      margin-inline: auto;
      padding: var(--mei-admin-form-pad);
      border: 1px solid var(--mei-color-border-default, rgba(148,163,184,.24));
      border-radius: 10px;
      background: rgba(15, 23, 42, .72);
      box-sizing: border-box;
    }
    .mei-admin-form-surface.mei-admin-form-surface--wide {
      --mei-admin-form-max-w: 780px;
    }
    .mei-admin-form-page > .mei-admin-form-surface { flex: 0 0 auto; }
    .mei-admin-form-surface > h2 { margin: 0 0 14px; color: var(--mei-color-text-primary, #fff); font-size: var(--mei-shell-font-3, 18px); font-weight: 600; }
    .mei-admin-form-group { display: grid; gap: 12px; margin: 0 0 18px; padding: 0 0 16px; border-bottom: 1px solid rgba(148,163,184,.14); }
    .mei-admin-form-group:last-of-type { border-bottom: 0; margin-bottom: 8px; padding-bottom: 0; }
    .mei-admin-form-group > h3 { margin: 0; color: var(--mei-color-text-primary, #fff); font-size: var(--mei-shell-font-1, 16px); font-weight: 600; }
    .mei-admin-form-group details > summary { cursor: pointer; color: var(--mei-color-text-muted, #94a3b8); font-size: var(--mei-shell-font-2, 14px); }
    @media (max-width: 520px) {
      .mei-admin-form-surface { min-width: 100%; width: 100%; }
    }
    .mei-admin-brick.mei-admin-explorer-root {
      max-width: none; width: 100%;
      flex: 1; min-height: 0; height: 100%; max-height: 100%;
      margin: 0; padding: 0;
      border: 1px solid rgba(148,163,184,.16); border-radius: 12px;
      background: rgba(15, 23, 42, .78);
      display: flex; flex-direction: column; overflow: hidden;
    }
    .mei-admin-brick h2 {
      margin: 0 0 12px; color: var(--mei-color-text-primary, #fff);
      font-size: var(--mei-shell-font-3, 18px); font-weight: 600;
    }
    .mei-admin-explorer-root > .mei-admin-explorer-header { display: none; }
    .mei-admin-form-card { display: grid; gap: 14px; }
    .mei-admin-field { display: grid; gap: 6px; color: var(--mei-color-text-body, #cbd5e1); }
    .mei-admin-field input, .mei-admin-field textarea { width: 100%; padding: 9px 11px; color: var(--mei-color-text-primary, #fff); border: 1px solid var(--mei-color-input-border, #334155); border-radius: 6px; background: var(--mei-color-input-bg, rgba(15,23,42,.8)); box-sizing: border-box; }
    .mei-admin-field textarea { min-height: 120px; font-family: ui-monospace, monospace; }
    .mei-admin-brick button { justify-self: start; padding: 7px 12px; font-size: var(--mei-shell-font-1, 16px); color: var(--mei-color-btn-primary-text, #041320); border: 0; border-radius: 6px; background: var(--mei-color-btn-primary-bg, #38bdf8); cursor: pointer; }
    .mei-admin-brick button:disabled { opacity: .55; cursor: wait; }
    .mei-admin-brick button.mei-admin-btn-secondary { background: transparent; color: var(--mei-color-text-body, #cbd5e1); border: 1px solid var(--mei-color-border-default, rgba(148,163,184,.35)); }
    .mei-admin-brick button.mei-admin-btn-danger { background: transparent; color: #fca5a5; border: 1px solid rgba(248,113,113,.45); }
    /* Splitter must NOT inherit .mei-admin-brick button primary styles. */
    .mei-admin-brick .mei-admin-explorer-splitter,
    .mei-admin-explorer-splitter {
      all: unset;
      box-sizing: border-box;
      position: relative;
      z-index: 3;
      display: block;
      width: 1px;
      height: 100%;
      margin: 0;
      padding: 0;
      border: 0;
      border-radius: 0;
      background: rgba(148, 163, 184, 0.2);
      cursor: col-resize;
      touch-action: none;
      flex: 0 0 1px;
    }
    .mei-admin-explorer-splitter::before {
      content: "";
      position: absolute;
      top: 0;
      bottom: 0;
      left: -4px;
      width: 9px;
      background: transparent;
    }
    .mei-admin-explorer-splitter:hover,
    .mei-admin-explorer-splitter.is-dragging,
    .mei-admin-explorer-splitter:focus-visible {
      background: rgba(148, 163, 184, 0.45);
      outline: none;
    }
    .mei-admin-data-grid { width: 100%; border-collapse: collapse; table-layout: fixed; }
    .mei-admin-data-grid th, .mei-admin-data-grid td { padding: 9px 10px; text-align: left; border-bottom: 1px solid var(--mei-color-table-row-border, rgba(148,163,184,.16)); vertical-align: top; }
    .mei-admin-data-grid th { color: var(--mei-color-text-muted, #94a3b8); font-weight: 500; font-size: var(--mei-shell-font-2, 14px); }
    .mei-admin-data-grid td { color: var(--mei-color-text-body, #cbd5e1); font-size: var(--mei-shell-font-1, 16px); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .mei-admin-data-grid td.mei-admin-cell-path { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: var(--mei-shell-font-2, 14px); white-space: nowrap; }
    .mei-admin-data-grid col.col-slot_id { width: 16%; }
    .mei-admin-data-grid col.col-status { width: 10%; }
    .mei-admin-data-grid col.col-kind { width: 8%; }
    .mei-admin-data-grid col.col-active_path { width: 36%; }
    .mei-admin-data-grid col.col-file_count { width: 8%; }
    .mei-admin-data-grid col.col-size_bytes { width: 10%; }
    .mei-admin-data-grid col.col-modified_ms { width: 12%; }
    .mei-admin-action-strip { display: flex; flex-wrap: wrap; gap: 10px; }
    mei-admin-navigator nav { display: grid; gap: 4px; }
    mei-admin-navigator nav a { padding: 7px 9px; color: var(--mei-color-text-body, #cbd5e1); text-decoration: none; border-radius: 5px; }
    mei-admin-navigator nav a[aria-current="page"] { color: #7dd3fc; background: rgba(14,116,144,.16); }
    .mei-admin-status { margin: 10px 0 0; color: var(--mei-color-text-muted, #94a3b8); font-size: var(--mei-shell-font-2, 14px); line-height: 1.5; }
    .mei-admin-status:empty { display: none; }
    .mei-admin-status[data-tone="error"] { color: var(--mei-color-status-error, #fca5a5); }
    .mei-admin-status[data-tone="ok"] { color: var(--mei-color-feedback-ok, #86efac); }
    .mei-admin-hint { margin: 0 0 12px; color: var(--mei-color-text-muted, #94a3b8); font-size: var(--mei-shell-font-2, 14px); line-height: 1.55; }
    .mei-admin-path-bar { margin: 0 0 14px; padding: 10px 12px; border-radius: 8px; background: rgba(30, 41, 59, .55); border: 1px solid rgba(148,163,184,.14); color: var(--mei-color-text-body, #cbd5e1); font-size: var(--mei-shell-font-2, 14px); font-family: ui-monospace, SFMono-Regular, Menlo, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .mei-admin-path-bar span { color: var(--mei-color-text-muted, #94a3b8); font-family: inherit; margin-right: 8px; }
    .mei-admin-file-list { display: grid; gap: 8px; margin-top: 4px; }
    .mei-admin-file-section { margin-top: 4px; }
    .mei-admin-file-section-title { margin: 0 0 8px; font-size: var(--mei-shell-font-2, 14px); letter-spacing: .04em; text-transform: uppercase; color: var(--mei-color-text-muted, #94a3b8); }
    .mei-admin-file-row { display: grid; grid-template-columns: minmax(0,1fr) auto; gap: 12px; align-items: center; padding: 11px 12px; border: 1px solid var(--mei-color-border-default, rgba(148,163,184,.16)); border-radius: 8px; background: rgba(15, 23, 42, .28); }
    .mei-admin-file-row.is-current { border-color: rgba(56, 189, 248, .4); background: rgba(14, 116, 144, .12); }
    .mei-admin-file-row.is-history { opacity: .94; }
    .mei-admin-file-meta { min-width: 0; color: var(--mei-color-text-body, #cbd5e1); }
    .mei-admin-file-meta strong { display: flex; align-items: center; gap: 8px; color: var(--mei-color-text-primary, #fff); overflow: hidden; }
    .mei-admin-file-meta strong .mei-admin-file-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .mei-admin-file-meta .mei-admin-file-sub { display: block; margin-top: 4px; font-size: var(--mei-shell-font-2, 14px); color: var(--mei-color-text-muted, #94a3b8); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
    .mei-admin-file-actions { display: inline-flex; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }
    .mei-admin-badge { display: inline-block; padding: 2px 8px; border-radius: 999px; font-size: var(--mei-shell-font-2, 14px); font-weight: 500; border: 1px solid rgba(56, 189, 248, .55); color: #7dd3fc; flex-shrink: 0; }
    .mei-admin-chip { display: inline-block; padding: 2px 8px; border-radius: 999px; font-size: var(--mei-shell-font-2, 14px); border: 1px solid rgba(148,163,184,.3); color: var(--mei-color-text-body, #cbd5e1); }
    .mei-admin-chip[data-tone="ready"] { border-color: rgba(134, 239, 172, .4); color: #86efac; background: rgba(22, 101, 52, .18); }
    .mei-admin-chip[data-tone="pending"] { border-color: rgba(251, 191, 36, .4); color: #fbbf24; background: rgba(120, 53, 15, .18); }
    .mei-admin-chip[data-tone="missing"] { border-color: rgba(248, 113, 113, .4); color: #fca5a5; background: rgba(127, 29, 29, .18); }
    .mei-admin-upload { margin-top: 16px; padding: 14px; border: 1px dashed rgba(148,163,184,.24); border-radius: 8px; background: rgba(15, 23, 42, .22); }
    .mei-admin-upload label { display: block; margin-bottom: 8px; font-size: var(--mei-shell-font-2, 14px); color: var(--mei-color-text-muted, #94a3b8); }
    .mei-admin-upload input[type="file"] { width: 100%; color: var(--mei-color-text-body, #cbd5e1); font-size: var(--mei-shell-font-1, 16px); }
    .mei-admin-explorer {
      --nav-width: 50%;
      flex: 1; min-height: 0; width: 100%; height: 100%;
      display: grid; grid-template-columns: minmax(280px, var(--nav-width)) 1px minmax(260px, 1fr);
      align-items: stretch; box-sizing: border-box;
    }
    .mei-admin-explorer-nav, .mei-admin-explorer-detail {
      min-width: 0; min-height: 0; height: 100%;
      display: flex; flex-direction: column; overflow: hidden; box-sizing: border-box;
    }
    .mei-admin-explorer-nav { padding: 12px; background: rgba(2, 6, 23, .18); }
    .mei-admin-explorer-detail { padding: 14px 16px; background: rgba(15, 23, 42, .28); }
    .mei-admin-explorer-detail .mei-admin-form-surface { margin-top: 4px; }
    .mei-admin-explorer.is-resizing { cursor: col-resize; user-select: none; }
    .mei-admin-explorer.is-resizing * { cursor: col-resize !important; user-select: none !important; }
    .mei-admin-explorer-toolbar { display: flex; align-items: center; gap: 8px; flex: 0 0 auto; margin-bottom: 10px; }
    .mei-admin-explorer-search {
      flex: 1; min-width: 0; padding: 8px 10px; color: var(--mei-color-text-primary, #fff);
      border: 1px solid rgba(71,85,105,.9); border-radius: 8px; background: rgba(15,23,42,.75);
      outline: none; font-size: var(--mei-shell-font-1, 16px);
    }
    .mei-admin-explorer-search:focus { border-color: rgba(56,189,248,.55); box-shadow: 0 0 0 2px rgba(14,116,144,.25); }
    .mei-admin-view-toggle {
      display: inline-flex; gap: 0; padding: 2px; border-radius: 8px;
      border: 1px solid rgba(71,85,105,.85); background: rgba(15,23,42,.55);
    }
    .mei-admin-view-toggle button {
      margin: 0; padding: 5px 10px; border: 0; border-radius: 6px;
      background: transparent; color: var(--mei-color-text-muted, #94a3b8);
    }
    .mei-admin-view-toggle button[aria-pressed="true"] {
      color: var(--mei-color-btn-primary-text, #041320); background: var(--mei-color-btn-primary-bg, #38bdf8);
    }
    .mei-admin-explorer-sort {
      flex: 0 0 auto; max-width: 148px; min-width: 112px; padding: 6px 8px;
      color: var(--mei-color-text-primary, #fff); font-size: var(--mei-shell-font-2, 14px);
      border: 1px solid rgba(71,85,105,.85); border-radius: 8px; background: rgba(15,23,42,.75);
    }
    .mei-admin-file-kind {
      display: inline-flex; align-items: center; gap: 6px; flex-shrink: 0;
    }
    .mei-admin-file-kind-icon {
      width: 18px; height: 18px; flex: 0 0 auto; color: #94a3b8;
    }
    .mei-admin-file-kind[data-file-kind="folder"] .mei-admin-file-kind-icon { color: #fbbf24; }
    .mei-admin-file-kind[data-file-kind="xlsx"] .mei-admin-file-kind-icon,
    .mei-admin-file-kind[data-file-kind="xls"] .mei-admin-file-kind-icon,
    .mei-admin-file-kind[data-file-kind="csv"] .mei-admin-file-kind-icon { color: #86efac; }
    .mei-admin-file-kind[data-file-kind="pdf"] .mei-admin-file-kind-icon { color: #fca5a5; }
    .mei-admin-file-kind[data-file-kind="doc"] .mei-admin-file-kind-icon,
    .mei-admin-file-kind[data-file-kind="docx"] .mei-admin-file-kind-icon { color: #7dd3fc; }
    .mei-admin-file-kind[data-file-kind="ppt"] .mei-admin-file-kind-icon,
    .mei-admin-file-kind[data-file-kind="pptx"] .mei-admin-file-kind-icon { color: #fdba74; }
    .mei-admin-file-kind[data-file-kind="image"] .mei-admin-file-kind-icon { color: #c4b5fd; }
    .mei-admin-file-kind[data-file-kind="video"] .mei-admin-file-kind-icon { color: #f9a8d4; }
    .mei-workspace-share-upload-panel {
      flex: 0 0 auto; display: grid; gap: 8px; margin-bottom: 12px;
      padding-bottom: 12px; border-bottom: 1px solid rgba(148,163,184,.16);
    }
    .mei-workspace-share-upload-panel .mei-workspace-share-breadcrumb { margin-bottom: 0; }
    .mei-workspace-share-upload-rows { display: grid; gap: 6px; }
    .mei-workspace-share-upload-row {
      display: flex; flex-wrap: wrap; align-items: center; gap: 8px; min-height: 32px;
    }
    .mei-workspace-share-upload-row input[type="file"] {
      flex: 1 1 180px; min-width: 0; max-width: 100%; color: var(--mei-color-text-body, #cbd5e1);
      font-size: var(--mei-shell-font-2, 14px);
    }
    .mei-workspace-share-upload-row .mei-admin-status {
      flex: 1 1 140px; margin: 0; font-size: var(--mei-shell-font-2, 14px);
    }
    .mei-workspace-share-selection-panel { min-width: 0; }
    .mei-workspace-share-selection-title {
      display: flex; align-items: center; gap: 10px; margin: 0 0 8px;
    }
    .mei-workspace-share-selection-title h2 {
      margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
      font-size: var(--mei-shell-font-3, 18px); color: var(--mei-color-text-primary, #fff);
    }
    .mei-admin-explorer-scroll { flex: 1; min-height: 0; overflow: auto; overscroll-behavior: contain; }
    .mei-admin-resource-collection { display: grid; gap: 8px; }
    .mei-admin-resource-collection[data-view="card"] {
      grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    }
    .mei-admin-resource-collection[data-view="list"] { grid-template-columns: minmax(0, 1fr); gap: 6px; }
    .mei-admin-resource-card {
      min-width: 0; padding: 11px 12px; border: 1px solid rgba(148,163,184,.16); border-radius: 10px;
      background: rgba(30,41,59,.42); cursor: pointer;
      transition: border-color .15s ease, background .15s ease, transform .15s ease;
    }
    .mei-admin-resource-card:hover { border-color: rgba(148,163,184,.34); background: rgba(51,65,85,.38); }
    .mei-admin-resource-card.is-selected {
      border-color: rgba(56,189,248,.55); background: rgba(14,116,144,.16);
      box-shadow: inset 0 0 0 1px rgba(56,189,248,.18);
    }
    .mei-admin-resource-card-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 8px; }
    .mei-admin-resource-card-title {
      display: flex; align-items: center; gap: 8px; min-width: 0;
    }
    .mei-admin-resource-card-title .mei-admin-file-kind { flex-shrink: 0; }
    .mei-admin-resource-card-title .mei-admin-file-kind-chip { display: none; }
    .mei-admin-resource-card-title h3 {
      margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
      color: var(--mei-color-text-primary, #fff); font-size: var(--mei-shell-font-1, 16px); font-weight: 600; line-height: 1.35;
    }
    .mei-admin-resource-current { margin: 8px 0 0; overflow: hidden; color: var(--mei-color-text-muted, #94a3b8); font: var(--mei-shell-font-2, 14px)/1.35 ui-monospace, SFMono-Regular, Menlo, monospace; text-overflow: ellipsis; white-space: nowrap; }
    .mei-admin-resource-meta-line { margin: 6px 0 0; color: #64748b; font-size: var(--mei-shell-font-2, 14px); }
    .mei-admin-resource-card[data-view="list"] {
      display: grid; grid-template-columns: minmax(96px, 140px) auto minmax(0, 1fr);
      align-items: center; gap: 10px 12px; padding: 9px 12px;
    }
    .mei-admin-resource-card[data-view="list"] .mei-admin-resource-card-head { display: contents; }
    .mei-admin-resource-card[data-view="list"] .mei-admin-resource-card-title { min-width: 0; }
    .mei-admin-resource-card[data-view="list"] .mei-admin-resource-current { margin: 0; }
    .mei-admin-resource-card[data-view="list"] .mei-admin-resource-meta-line { display: none; }
    .mei-admin-resource-card[data-view="list"].is-selected { box-shadow: inset 3px 0 0 rgba(56,189,248,.75); }
    .mei-admin-brick.is-embedded { max-width: none; margin: 0; padding: 0; border: 0; border-radius: 0; background: transparent; height: auto; }
    .mei-admin-brick.is-embedded > h2 { margin: 0 0 12px; font-size: var(--mei-shell-font-3, 18px); }
    .mei-admin-explorer-empty { padding: 28px 16px; text-align: center; color: var(--mei-color-text-muted, #94a3b8); border: 1px dashed rgba(148,163,184,.2); border-radius: 10px; font-size: var(--mei-shell-font-1, 16px); }
    .mei-admin-explorer-detail-empty { display: grid; place-items: center; min-height: 100%; color: var(--mei-color-text-muted, #94a3b8); font-size: var(--mei-shell-font-1, 16px); border: 1px dashed rgba(148,163,184,.18); border-radius: 10px; }
    .mei-workspace-share-page {
      box-sizing: border-box;
      width: 100%; height: 100%;
      max-height: 100%;
      min-height: 0; margin: 0; padding: 0;
      display: flex; flex-direction: column; overflow: hidden;
    }
    .mei-workspace-page--fill {
      flex: 1; min-height: 0; height: 100%;
      display: flex; flex-direction: column; overflow: hidden;
    }
    .mei-workspace-share-page > header { display: none; }
    .mei-workspace-share-page mei-workspace-share {
      flex: 1; min-height: 0; width: 100%; height: 100%;
      display: flex; flex-direction: column;
    }
    .mei-workspace-share.mei-admin-explorer {
      --nav-width: 42%;
      margin: 0;
    }
    .mei-workspace-share-nav {
      display: flex; flex-direction: column; gap: 2px;
      flex: 0 1 auto; max-height: 36%; min-height: 0; overflow: auto;
    }
    .mei-workspace-share-nav h2 {
      margin: 0 0 8px; font-size: var(--mei-shell-font-2, 14px);
      color: var(--mei-color-text-muted, #94a3b8); font-weight: 500;
      text-transform: uppercase; letter-spacing: .04em;
    }
    .mei-workspace-share-nav button {
      display: block; width: 100%; margin: 0; padding: 7px 9px; overflow: hidden;
      color: var(--mei-color-text-body, #cbd5e1); text-align: left;
      text-overflow: ellipsis; white-space: nowrap; border: 0; border-radius: 5px; background: transparent;
    }
    .mei-workspace-share-nav button[aria-current="true"] { color: #7dd3fc; background: rgba(14,116,144,.16); }
    .mei-workspace-share-main {
      min-width: 0; min-height: 0; height: 100%;
      display: flex; flex-direction: column; overflow: hidden;
    }
    .mei-workspace-share-breadcrumb {
      display: flex; gap: 6px; align-items: center; flex: 0 0 auto;
      margin: 0 0 10px; color: var(--mei-color-text-muted, #94a3b8);
      font-size: var(--mei-shell-font-2, 14px);
    }
    .mei-workspace-share-breadcrumb button { padding: 2px 4px; color: #7dd3fc; border: 0; background: transparent; }
    .mei-workspace-share-entry-actions { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 10px; }
    .mei-workspace-share-entry-actions button, .mei-workspace-share-entry-actions a {
      padding: 5px 8px; color: var(--mei-color-text-body, #cbd5e1);
      font-size: var(--mei-shell-font-2, 14px); text-decoration: none;
      border: 1px solid rgba(148,163,184,.3); border-radius: 5px; background: transparent;
    }
    .mei-workspace-share-detail-panel {
      width: min(100%, var(--mei-admin-form-max-w, 640px));
      min-width: min(100%, var(--mei-admin-form-min-w, 420px));
      margin-inline: auto; padding: var(--mei-admin-form-pad, 20px 24px);
      border: 1px solid rgba(148,163,184,.2); border-radius: 10px;
      background: rgba(15, 23, 42, .55); box-sizing: border-box;
      display: flex; flex-direction: column; min-height: 0;
    }
    @media (max-width: 900px) {
      .mei-admin-explorer { grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(200px, 40%) 1px minmax(0, 1fr); }
      .mei-admin-brick .mei-admin-explorer-splitter,
      .mei-admin-explorer-splitter {
        width: 100%; height: 1px; flex: 0 0 1px; cursor: row-resize;
      }
      .mei-admin-explorer-splitter::before { left: 0; right: 0; top: -4px; width: auto; height: 9px; }
      .mei-admin-explorer-toolbar { align-items: stretch; flex-direction: column; }
    }
  `;
}

class AdminBrick extends HTMLElement {
  static observedAttributes = ["data-props"];

  connectedCallback() {
    ensureAdminStyles();
    this.style.display = "flex";
    this.style.flexDirection = "column";
    this.style.flex = "1 1 auto";
    this.style.minHeight = "0";
    this.style.width = "100%";
    this.style.height = "100%";
    this.style.boxSizing = "border-box";
    this.update();
  }

  attributeChangedCallback(name, oldValue, newValue) {
    if (name === "data-props" && oldValue !== newValue && this.isConnected) {
      this.update();
    }
  }

  update() {
    const props = parseProps(this);
    this.render(props);
    if (typeof this.hydrate === "function") {
      void this.hydrate(props).catch((error) => this.showError(error));
    }
  }

  showError(error) {
    const root = this.querySelector(".mei-admin-brick") || this;
    const status = element("p", { className: "mei-admin-status", text: error.message || error });
    status.dataset.tone = "error";
    root.append(status);
    console.error("[admin-brick] provider request failed", error);
  }

  reset(title) {
    const root = element("section", { className: "mei-admin-brick" });
    if (title) root.append(element("h2", { text: title }));
    this.replaceChildren(root);
    return root;
  }

  bindSplitter(splitter, explorer) {
    const applyWidth = (pct) => {
      const next = Math.min(72, Math.max(28, pct));
      this._navWidthPct = next;
      explorer.style.setProperty("--nav-width", `${next}%`);
    };
    applyWidth(this._navWidthPct || readStoredNavWidth());

    const onPointerMove = (event) => {
      if (!this._resizing) return;
      const rect = explorer.getBoundingClientRect();
      if (!rect.width) return;
      applyWidth(((event.clientX - rect.left) / rect.width) * 100);
    };
    const onPointerUp = () => {
      if (!this._resizing) return;
      this._resizing = false;
      splitter.classList.remove("is-dragging");
      explorer.classList.remove("is-resizing");
      writeStoredNavWidth(this._navWidthPct);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };
    splitter.addEventListener("pointerdown", (event) => {
      if (event.button != null && event.button !== 0) return;
      event.preventDefault();
      this._resizing = true;
      splitter.classList.add("is-dragging");
      explorer.classList.add("is-resizing");
      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp);
    });
    splitter.addEventListener("keydown", (event) => {
      const step = event.shiftKey ? 5 : 2;
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        applyWidth((this._navWidthPct || 42) - step);
        writeStoredNavWidth(this._navWidthPct);
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        applyWidth((this._navWidthPct || 42) + step);
        writeStoredNavWidth(this._navWidthPct);
      }
    });
  }
}

function appendFormFields(form, fields, payload, readonly) {
  fields.forEach((field) => {
    const spec = typeof field === "string" ? { name: field, label: field } : field || {};
    const name = String(spec.name || spec.id || "").trim();
    if (!name) return;
    const value = payload[name] ?? spec.default ?? "";
    const label = element("label", { className: "mei-admin-field" });
    label.append(element("span", { text: spec.label || name }));
    const input =
      spec.multiline || (value && typeof value === "object")
        ? element("textarea")
        : element("input");
    input.name = name;
    input.dataset.valueKind = value && typeof value === "object" ? "json" : "string";
    input.value =
      input.dataset.valueKind === "json"
        ? JSON.stringify(value, null, 2)
        : value == null
          ? ""
          : String(value);
    input.disabled = readonly;
    label.append(input);
    form.append(label);
  });
}

function readFormFieldValues(form) {
  const detail = {};
  form.querySelectorAll("[name]").forEach((input) => {
    detail[input.name] =
      input.dataset.valueKind === "json" ? JSON.parse(input.value || "{}") : input.value;
  });
  return detail;
}

function pathGet(source, path) {
  if (!path) return source;
  return String(path)
    .split(".")
    .filter(Boolean)
    .reduce((acc, key) => (acc && typeof acc === "object" ? acc[key] : undefined), source);
}

function pathSet(target, path, value) {
  const keys = String(path || "")
    .split(".")
    .filter(Boolean);
  if (!keys.length) return value;
  let cursor = target;
  keys.slice(0, -1).forEach((key) => {
    if (!cursor[key] || typeof cursor[key] !== "object") cursor[key] = {};
    cursor = cursor[key];
  });
  cursor[keys[keys.length - 1]] = value;
  return target;
}

function deepClone(value) {
  return JSON.parse(JSON.stringify(value ?? {}));
}

class FormCard extends AdminBrick {
  async hydrate(props) {
    if (!providerRefId(props.payload_provider)) return;
    const response = await readProvider(props.payload_provider);
    this._revision = Number(response.revision || 0);
    this.render({ ...props, payload: response.payload || {} });
  }

  render(props) {
    const embedded = props.embedded === true;
    const root = this.reset("");
    if (embedded) root.classList.add("is-embedded");
    else root.classList.add("mei-admin-form-page");
    const surface = element("div", { className: "mei-admin-form-surface" });
    if (props.title) surface.append(element("h2", { text: props.title }));
    const form = element("form", { className: "mei-admin-form-card" });
    const readonly = props.readonly === true || props.mode === "readonly";
    const payload = props.payload && typeof props.payload === "object" ? props.payload : {};
    const fields = Array.isArray(props.fields) && props.fields.length
      ? props.fields
      : Object.keys(payload);
    appendFormFields(form, fields, payload, readonly);
    const status = element("p", { className: "mei-admin-status" });
    let save = null;
    if (!readonly) {
      const actions = element("div", { className: "mei-admin-action-strip" });
      save = element("button", { text: props.submit_label || "保存", type: "submit" });
      const cancel = element("button", {
        text: "取消更改",
        type: "button",
        className: "mei-admin-btn-secondary",
      });
      cancel.addEventListener("click", () => {
        this._dirty = false;
        this.render({ ...props, payload });
        this.dispatchEvent(
          new CustomEvent("mei:admin-dirty-change", {
            bubbles: true,
            composed: true,
            detail: { dirty: false },
          }),
        );
      });
      actions.append(save, cancel);
      form.append(actions);
      form.querySelectorAll("[name]").forEach((input) => {
        input.addEventListener("input", () => {
          if (this._dirty) return;
          this._dirty = true;
          status.textContent = "有未保存的更改";
          this.dispatchEvent(
            new CustomEvent("mei:admin-dirty-change", {
              bubbles: true,
              composed: true,
              detail: { dirty: true },
            }),
          );
        });
      });
    }
    form.append(status);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      if (readonly || !save) return;
      try {
        const detail = readFormFieldValues(form);
        save.disabled = true;
        status.textContent = "正在保存…";
        if (providerRefId(props.submit_provider)) {
          const response = await putConfigRecord(
            props.submit_provider,
            detail,
            Number(this._revision || 0),
          );
          this._revision = Number(response.revision || this._revision || 0);
        }
        this._dirty = false;
        status.textContent = "已保存";
        status.dataset.tone = "ok";
        this.dispatchEvent(
          new CustomEvent("mei:admin-submit", { bubbles: true, composed: true, detail }),
        );
      } catch (error) {
        status.textContent = error.message || String(error);
        status.dataset.tone = "error";
        console.error("[admin.form-card] submit failed", error);
      } finally {
        save.disabled = false;
      }
    });
    surface.append(form);
    root.append(surface);
  }
}

class GroupedForm extends AdminBrick {
  groupKey(group, index) {
    return String(group?.id || group?.title || `group-${index}`);
  }

  async hydrate(props) {
    const groups = Array.isArray(props.groups) ? props.groups : [];
    this._groupRevisions = {};
    this._groupPayloads = {};
    const seen = new Set();
    for (let index = 0; index < groups.length; index += 1) {
      const group = groups[index];
      if (!group || group.rest) continue;
      const getRef = group.payload_provider || props.payload_provider;
      if (!providerRefId(getRef)) continue;
      const cacheKey = providerRefId(getRef);
      const key = this.groupKey(group, index);
      if (!seen.has(cacheKey)) {
        const response = await readProvider(getRef);
        seen.add(cacheKey);
        this._providerCache = this._providerCache || {};
        this._providerCache[cacheKey] = {
          revision: Number(response.revision || 0),
          payload: deepClone(response.payload || {}),
        };
      }
      const cached = this._providerCache[cacheKey];
      this._groupRevisions[key] = cached.revision;
      this._groupPayloads[key] = deepClone(cached.payload);
    }
    if (
      !groups.length &&
      providerRefId(props.payload_provider)
    ) {
      const response = await readProvider(props.payload_provider);
      this._revision = Number(response.revision || 0);
      this.render({ ...props, payload: response.payload || {} });
      return;
    }
    this.render(props);
  }

  fitFormPageHeight() {
    if (typeof this.fitExplorerHeight === "function") {
      this.fitExplorerHeight();
      return;
    }
    const footerReserve = 52;
    const top = Math.max(0, Math.round(this.getBoundingClientRect().top));
    const height = Math.max(360, Math.floor(window.innerHeight - top - footerReserve));
    this.style.setProperty("height", `${height}px`, "important");
    this.style.setProperty("max-height", `${height}px`, "important");
    this.style.setProperty("min-height", "0", "important");
    this.style.setProperty("flex", "1 1 auto", "important");
    this.style.setProperty("display", "flex", "important");
    this.style.setProperty("flex-direction", "column", "important");
    const page = this.querySelector(".mei-admin-form-page");
    if (page instanceof HTMLElement) {
      page.style.setProperty("flex", "1 1 auto", "important");
      page.style.setProperty("min-height", "0", "important");
      page.style.setProperty("height", "100%", "important");
      page.style.setProperty("overflow", "auto", "important");
    }
  }

  render(props) {
    const root = this.reset("");
    root.classList.add("mei-admin-form-page");
    const surface = element("div", {
      className: "mei-admin-form-surface mei-admin-form-surface--wide",
    });
    if (props.title) surface.append(element("h2", { text: props.title }));
    const form = element("form", { className: "mei-admin-form-card" });
    const readonly = props.readonly === true || props.mode === "readonly";
    const groups = Array.isArray(props.groups) ? props.groups : [];
    const legacyPayload =
      props.payload && typeof props.payload === "object" ? deepClone(props.payload) : {};

    groups.forEach((group, index) => {
      if (!group || group.rest) return;
      const key = this.groupKey(group, index);
      const payload =
        this._groupPayloads?.[key] != null ? deepClone(this._groupPayloads[key]) : legacyPayload;
      const section = element("section", { className: "mei-admin-form-group" });
      section.dataset.groupKey = key;
      section.append(element("h3", { text: group.title || group.id || `分组 ${index + 1}` }));
      const paths = Array.isArray(group.paths)
        ? group.paths
        : group.path
          ? [group.path]
          : [];
      paths.forEach((path) => {
        const value = pathGet(payload, path);
        if (value && typeof value === "object" && !Array.isArray(value)) {
          Object.keys(value).forEach((fieldKey) => {
            const leaf = value[fieldKey];
            const fieldName = `${path}.${fieldKey}`;
            const label = element("label", { className: "mei-admin-field" });
            label.append(
              element("span", {
                text: group.field_labels?.[fieldKey] || fieldKey,
              }),
            );
            const input =
              leaf && typeof leaf === "object" ? element("textarea") : element("input");
            input.name = fieldName;
            input.dataset.groupKey = key;
            input.dataset.valueKind = leaf && typeof leaf === "object" ? "json" : "string";
            input.dataset.path = fieldName;
            input.value =
              input.dataset.valueKind === "json"
                ? JSON.stringify(leaf, null, 2)
                : leaf == null
                  ? ""
                  : String(leaf);
            input.disabled = readonly;
            label.append(input);
            section.append(label);
          });
        } else {
          const label = element("label", { className: "mei-admin-field" });
          label.append(element("span", { text: group.field_labels?.[path] || path }));
          const input = element("input");
          input.name = path;
          input.dataset.groupKey = key;
          input.dataset.path = path;
          input.dataset.valueKind = "string";
          input.value = value == null ? "" : String(value);
          input.disabled = readonly;
          label.append(input);
          section.append(label);
        }
      });
      if (Array.isArray(group.fields) && group.fields.length) {
        group.fields.forEach((field) => {
          const spec = typeof field === "string" ? { name: field, label: field } : field || {};
          const name = String(spec.name || spec.id || "").trim();
          if (!name) return;
          const value = payload[name] ?? spec.default ?? "";
          const label = element("label", { className: "mei-admin-field" });
          label.append(element("span", { text: spec.label || name }));
          const input =
            spec.multiline || (value && typeof value === "object")
              ? element("textarea")
              : element("input");
          input.name = name;
          input.dataset.groupKey = key;
          input.dataset.path = name;
          input.dataset.valueKind = value && typeof value === "object" ? "json" : "string";
          input.value =
            input.dataset.valueKind === "json"
              ? JSON.stringify(value, null, 2)
              : value == null
                ? ""
                : String(value);
          input.disabled = readonly;
          label.append(input);
          section.append(label);
        });
      }
      form.append(section);
    });

    const status = element("p", { className: "mei-admin-status" });
    let save = null;
    if (!readonly) {
      const actions = element("div", { className: "mei-admin-action-strip" });
      save = element("button", { text: props.submit_label || "保存", type: "submit" });
      const cancel = element("button", {
        text: "取消更改",
        type: "button",
        className: "mei-admin-btn-secondary",
      });
      cancel.addEventListener("click", () => {
        this._dirty = false;
        void this.hydrate(props);
      });
      actions.append(save, cancel);
      form.append(actions);
      form.querySelectorAll("[name]").forEach((input) => {
        input.addEventListener("input", () => {
          if (this._dirty) return;
          this._dirty = true;
          status.textContent = "有未保存的更改";
        });
      });
    }
    form.append(status);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      if (readonly || !save) return;
      try {
        save.disabled = true;
        status.textContent = "正在保存…";
        for (let index = 0; index < groups.length; index += 1) {
          const group = groups[index];
          if (!group || group.rest) continue;
          const key = this.groupKey(group, index);
          const putRef = group.submit_provider || props.submit_provider;
          if (!providerRefId(putRef)) continue;
          const base = deepClone(this._groupPayloads?.[key] || {});
          form.querySelectorAll(`[name][data-group-key="${key}"]`).forEach((input) => {
            const path = input.dataset.path || input.name;
            const value =
              input.dataset.valueKind === "json"
                ? JSON.parse(input.value || "null")
                : input.value;
            pathSet(base, path, value);
          });
          // When group.path is set (e.g. font), PUT merges into full theme payload:
          // base already is full payload from provider; pathSet updated nested keys.
          const response = await putConfigRecord(
            putRef,
            base,
            Number(this._groupRevisions?.[key] || 0),
          );
          this._groupRevisions[key] = Number(response.revision || this._groupRevisions?.[key] || 0);
          this._groupPayloads[key] = deepClone(base);
          const cacheKey = providerRefId(group.payload_provider || props.payload_provider);
          if (cacheKey && this._providerCache?.[cacheKey]) {
            this._providerCache[cacheKey] = {
              revision: this._groupRevisions[key],
              payload: deepClone(base),
            };
          }
        }
        this._dirty = false;
        status.textContent = "已保存";
        status.dataset.tone = "ok";
      } catch (error) {
        status.textContent = error.message || String(error);
        status.dataset.tone = "error";
        console.error("[admin.grouped-form] submit failed", error);
      } finally {
        save.disabled = false;
      }
    });
    surface.append(form);
    root.append(surface);
    queueMicrotask(() => this.fitFormPageHeight());
    requestAnimationFrame(() => this.fitFormPageHeight());
    setTimeout(() => this.fitFormPageHeight(), 50);
  }
}

function normalizeSlotRow(row, index) {
  const normalized = Object.fromEntries(
    Object.entries(row || {}).map(([key, value]) => [
      key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`),
      value,
    ]),
  );
  const files = Array.isArray(row?.files) ? row.files : normalized.files || [];
  return {
    ...normalized,
    slot_id: normalized.slot_id || normalized.id || `row-${index}`,
    active_path: normalized.active_path || "",
    status: normalized.status || "",
    kind: normalized.kind || "",
    size_bytes: normalized.size_bytes ?? "",
    modified_ms: normalized.modified_ms ?? "",
    file_count: files.length,
    files,
  };
}

const COLUMN_LABELS = {
  slot_id: "槽位",
  status: "状态",
  kind: "类型",
  active_path: "当前 path",
  file_count: "文件数",
  size_bytes: "大小",
  modified_ms: "修改时间",
};

function resourceMatchesQuery(resource, needle) {
  if (!needle) return true;
  const slot = resource.slot || {};
  const files = Array.isArray(slot.files) ? slot.files : [];
  const haystack = [
    resource.id,
    resource.title,
    resource.summary,
    slot.kind,
    slot.status,
    slot.activePath || slot.active_path,
    ...files.flatMap((file) => [file?.name, file?.path]),
  ];
  return haystack.some((value) =>
    String(value || "")
      .toLocaleLowerCase()
      .includes(needle),
  );
}

function readStoredNavWidth() {
  try {
    const raw = Number(window.localStorage.getItem("mei.admin.explorer.navWidth"));
    if (Number.isFinite(raw) && raw >= 28 && raw <= 72) return raw;
  } catch {
    /* ignore */
  }
  return 50;
}

function writeStoredNavWidth(value) {
  try {
    window.localStorage.setItem("mei.admin.explorer.navWidth", String(value));
  } catch {
    /* ignore */
  }
}

class CollectionView extends AdminBrick {
  async hydrate(props) {
    const specs = Array.isArray(props.resources) ? props.resources : [];
    const resources = await Promise.all(
      specs.map(async (spec, index) => {
        const id = String(spec?.id || spec?.slot_id || spec?.slotId || `resource-${index}`);
        const listRef = spec?.list_provider || spec?.listProvider;
        if (!providerRefId(listRef)) return { ...spec, id, slot: spec?.slot || null };
        const payload = await readProvider(listRef);
        const slots = Array.isArray(payload?.slots) ? payload.slots : [];
        const slot =
          slots.find((entry) => String(entry.slotId || entry.slot_id || "") === id) ||
          (slots.length === 1 ? slots[0] : null);
        return { ...spec, id, slot };
      }),
    );
    this._resources = resources;
    this.render({ ...props, resources });
  }

  initializeState(props) {
    if (this._explorerInitialized) return;
    const query = new URLSearchParams(window.location.search);
    this._query = query.get("q") || "";
    this._viewMode = query.get("view") || props.default_view || props.defaultView || "card";
    this._selectedId = query.get("sel") || "";
    this._sort = parseExplorerSort(query.get("sort"), "title");
    this._navWidthPct = readStoredNavWidth();
    this._explorerInitialized = true;
  }

  syncExplorerUrl() {
    updateExplorerUrl({
      q: this._query,
      view: this._viewMode,
      sel: this._selectedId,
      sort: formatExplorerSort(this._sort || { field: "title", dir: "asc" }),
    });
  }

  setSelection(id, props) {
    if (this._selectedId === id) return;
    this._selectedId = id || "";
    this.syncExplorerUrl();
    this.dispatchEvent(
      new CustomEvent("mei:admin-selection-change", {
        bubbles: true,
        composed: true,
        detail: {
          primaryId: this._selectedId || null,
          ids: this._selectedId ? [this._selectedId] : [],
          source: "admin.collection-view",
        },
      }),
    );
    this.paintSelection();
    this.paintDetail(props);
  }

  paintSelection() {
    this.querySelectorAll(".mei-admin-resource-card[data-resource-id]").forEach((card) => {
      const selected = card.getAttribute("data-resource-id") === this._selectedId;
      card.classList.toggle("is-selected", selected);
      card.setAttribute("aria-selected", String(selected));
    });
  }

  paintDetail(props) {
    const detailScroll = this.querySelector(
      ".mei-admin-explorer-detail > .mei-admin-explorer-scroll",
    );
    if (!detailScroll) {
      this.render({ ...props, resources: this._resources || props.resources || [] });
      return;
    }
    const resources = Array.isArray(this._resources)
      ? this._resources
      : Array.isArray(props.resources)
        ? props.resources
        : [];
    const needle = String(this._query || "").trim().toLocaleLowerCase();
    const filtered = sortCollectionResources(
      resources.filter((resource) => resourceMatchesQuery(resource, needle)),
      this._sort || { field: "title", dir: "asc" },
    );
    const selectedResource =
      filtered.find((resource) => String(resource.id || "") === this._selectedId) || null;
    detailScroll.replaceChildren();
    if (selectedResource) {
      const assetSlot = document.createElement("mei-admin-asset-slot");
      assetSlot.setAttribute(
        "data-props",
        JSON.stringify({
          title: selectedResource.title || selectedResource.id,
          slot_id: selectedResource.id,
          accept: selectedResource.accept,
          hint: selectedResource.hint || "",
          list_provider: selectedResource.list_provider || selectedResource.listProvider,
          replace_provider: selectedResource.replace_provider || selectedResource.replaceProvider,
          slot: selectedResource.slot,
          embedded: true,
        }),
      );
      detailScroll.append(assetSlot);
    } else {
      detailScroll.append(
        element("div", {
          className: "mei-admin-explorer-detail-empty",
          text: "选择左侧资源以编辑",
        }),
      );
    }
  }

  applySearch(value, props, resources) {
    this._query = value;
    this.syncExplorerUrl();
    this.render({ ...props, resources });
    queueMicrotask(() => {
      const next = this.querySelector(".mei-admin-explorer-search");
      if (next instanceof HTMLInputElement) {
        next.focus();
        const caret = next.value.length;
        next.setSelectionRange(caret, caret);
      }
    });
  }

  applySort(sort, props) {
    this._sort = sort || { field: "title", dir: "asc" };
    this.syncExplorerUrl();
    this.paintNavList(props);
    this.paintSelection();
  }

  filteredResources(props) {
    const resources = Array.isArray(this._resources)
      ? this._resources
      : Array.isArray(props.resources)
        ? props.resources
        : [];
    const needle = String(this._query || "").trim().toLocaleLowerCase();
    return sortCollectionResources(
      resources.filter((resource) => resourceMatchesQuery(resource, needle)),
      this._sort || { field: "title", dir: "asc" },
    );
  }

  paintNavList(props) {
    const navScroll = this.querySelector(".mei-admin-explorer-nav > .mei-admin-explorer-scroll");
    if (!navScroll) {
      this.render({ ...props, resources: this._resources || props.resources || [] });
      return;
    }
    const filtered = this.filteredResources(props);
    const selectedStillVisible = filtered.some(
      (resource) => String(resource.id || "") === this._selectedId,
    );
    if (!filtered.length) {
      if (this._selectedId) {
        this._selectedId = "";
        this.syncExplorerUrl();
      }
    } else if (!this._selectedId || !selectedStillVisible) {
      this._selectedId = String(filtered[0].id || "");
      this.syncExplorerUrl();
    }
    const collection = this.buildResourceCollection(filtered, props);
    navScroll.replaceChildren(collection);
  }

  buildResourceCollection(filtered, props) {
    const collection = element("div", { className: "mei-admin-resource-collection" });
    collection.dataset.view = this._viewMode === "list" ? "list" : "card";
    collection.setAttribute("role", "listbox");
    collection.setAttribute("aria-label", props.title || "资源");
    const needle = String(this._query || "").trim().toLocaleLowerCase();
    filtered.forEach((resource) => {
      const slot = resource.slot || {};
      const files = Array.isArray(slot.files) ? slot.files : [];
      const current = files.find((file) => file.isCurrent || file.is_current) || null;
      const activePath = slot.activePath || slot.active_path || "";
      const id = String(resource.id || slot.slotId || slot.slot_id || "");
      const selected = this._selectedId === id;
      const card = element("article", {
        className: selected ? "mei-admin-resource-card is-selected" : "mei-admin-resource-card",
      });
      card.dataset.resourceId = id;
      card.dataset.view = collection.dataset.view;
      card.setAttribute("role", "option");
      card.setAttribute("aria-selected", String(selected));
      card.tabIndex = 0;
      card.addEventListener("click", () => this.setSelection(id, props));
      card.addEventListener("keydown", (event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          this.setSelection(id, props);
        }
      });

      const head = element("div", { className: "mei-admin-resource-card-head" });
      const title = element("div", { className: "mei-admin-resource-card-title" });
      const currentName = current?.name || activePath || "";
      if (currentName) {
        title.append(fileKindIcon(resolveFileKind(currentName)));
      }
      title.append(element("h3", { text: resource.title || id }));
      head.append(title);
      const chip = element("span", {
        className: "mei-admin-chip",
        text: statusLabel(slot.status),
      });
      chip.dataset.tone = String(slot.status || "").toLowerCase();
      head.append(chip);
      card.append(head);

      const currentText =
        current?.name || activePath || (this._viewMode === "list" ? "—" : "尚未设置当前文件");
      const currentLine = element("p", {
        className: "mei-admin-resource-current",
        text: currentText,
      });
      currentLine.title = activePath || currentText;
      card.append(currentLine);
      if (this._viewMode !== "list") {
        card.append(
          element("p", {
            className: "mei-admin-resource-meta-line",
            text: `${files.length} 个文件`,
          }),
        );
      }
      collection.append(card);
    });
    if (!filtered.length) {
      collection.append(
        element("div", {
          className: "mei-admin-explorer-empty",
          text: needle ? "没有匹配的资源" : "暂无资源",
        }),
      );
    }
    return collection;
  }

  bindSplitter(splitter, explorer) {
    const applyWidth = (pct) => {
      const next = Math.min(72, Math.max(28, pct));
      this._navWidthPct = next;
      explorer.style.setProperty("--nav-width", `${next}%`);
    };
    applyWidth(this._navWidthPct || readStoredNavWidth());

    const onPointerMove = (event) => {
      if (!this._resizing) return;
      const rect = explorer.getBoundingClientRect();
      if (!rect.width) return;
      applyWidth(((event.clientX - rect.left) / rect.width) * 100);
    };
    const onPointerUp = () => {
      if (!this._resizing) return;
      this._resizing = false;
      splitter.classList.remove("is-dragging");
      explorer.classList.remove("is-resizing");
      writeStoredNavWidth(this._navWidthPct);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };
    splitter.addEventListener("pointerdown", (event) => {
      if (event.button != null && event.button !== 0) return;
      event.preventDefault();
      this._resizing = true;
      splitter.classList.add("is-dragging");
      explorer.classList.add("is-resizing");
      window.addEventListener("pointermove", onPointerMove);
      window.addEventListener("pointerup", onPointerUp);
    });
    splitter.addEventListener("keydown", (event) => {
      const step = event.shiftKey ? 5 : 2;
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        applyWidth((this._navWidthPct || 42) - step);
        writeStoredNavWidth(this._navWidthPct);
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        applyWidth((this._navWidthPct || 42) + step);
        writeStoredNavWidth(this._navWidthPct);
      }
    });
  }

  fitExplorerHeight() {
    const footerReserve = 52;
    const gutter = 12;
    const host =
      this.closest("#mei-compose-root") || this.closest(".mei-compose-document-host");
    if (!(host instanceof HTMLElement)) {
      const top = Math.max(0, Math.round(this.getBoundingClientRect().top));
      const height = Math.max(360, Math.floor(window.innerHeight - top - footerReserve));
      this.style.setProperty("height", `${height}px`, "important");
      this.style.setProperty("max-height", `${height}px`, "important");
      return;
    }

    const top = Math.max(0, Math.round(host.getBoundingClientRect().top));
    const hostHeight = Math.max(360, Math.floor(window.innerHeight - top - footerReserve));
    host.style.setProperty("box-sizing", "border-box", "important");
    host.style.setProperty("height", `${hostHeight}px`, "important");
    host.style.setProperty("max-height", `${hostHeight}px`, "important");
    host.style.setProperty("min-height", `${hostHeight}px`, "important");
    host.style.setProperty("overflow", "hidden", "important");
    host.style.setProperty("display", "flex", "important");
    host.style.setProperty("flex-direction", "column", "important");
    host.style.setProperty("padding", `${gutter}px`, "important");
    host.classList.remove("overflow-auto");
    host.classList.add("overflow-hidden");

    const tree =
      host.querySelector(":scope > .mei-structure-tree") ||
      this.closest(".mei-structure-tree");
    if (tree instanceof HTMLElement) {
      tree.style.setProperty("box-sizing", "border-box", "important");
      tree.style.setProperty("flex", "1 1 auto", "important");
      tree.style.setProperty("min-height", "0", "important");
      tree.style.setProperty("height", "100%", "important");
      tree.style.setProperty("max-height", "100%", "important");
      tree.style.setProperty("overflow", "hidden", "important");
      tree.style.setProperty("display", "flex", "important");
      tree.style.setProperty("flex-direction", "column", "important");
      tree.style.setProperty("width", "100%", "important");
    }

    // Materializer leaves compose nodes at height:auto; force the full chain to stretch.
    let node = this;
    while (node && node !== host) {
      if (node instanceof HTMLElement) {
        node.style.setProperty("box-sizing", "border-box", "important");
        node.style.setProperty("min-height", "0", "important");
        node.style.setProperty("width", "100%", "important");
        node.style.setProperty("max-width", "100%", "important");
        node.style.setProperty("overflow", "hidden", "important");
        node.style.setProperty("display", "flex", "important");
        node.style.setProperty("flex-direction", "column", "important");
        node.style.setProperty("flex", "1 1 auto", "important");
        node.style.setProperty("height", "100%", "important");
        node.style.setProperty("max-height", "100%", "important");
        node.style.setProperty("align-self", "stretch", "important");
      }
      if (node === tree) break;
      node = node.parentElement;
    }

    const inner =
      tree instanceof HTMLElement
        ? Math.floor(tree.getBoundingClientRect().height)
        : Math.floor(hostHeight - gutter * 2);
    if (inner > 120) {
      this.style.setProperty("height", `${inner}px`, "important");
      this.style.setProperty("max-height", `${inner}px`, "important");
      this.style.setProperty("min-height", `${inner}px`, "important");
    }

    const root = this.querySelector(".mei-admin-explorer-root");
    if (root instanceof HTMLElement) {
      root.style.setProperty("flex", "1 1 auto", "important");
      root.style.setProperty("min-height", "0", "important");
      root.style.setProperty("height", "100%", "important");
      root.style.setProperty("max-height", "100%", "important");
      root.style.setProperty("overflow", "hidden", "important");
      root.style.setProperty("display", "flex", "important");
      root.style.setProperty("flex-direction", "column", "important");
    }

    if (!this._fitBound) {
      this._fitBound = true;
      this._onFitResize = () => this.fitExplorerHeight();
      window.addEventListener("resize", this._onFitResize);
    }
  }

  disconnectedCallback() {
    if (this._onFitResize) {
      window.removeEventListener("resize", this._onFitResize);
      this._onFitResize = null;
      this._fitBound = false;
    }
  }

  render(props) {
    this.initializeState(props);
    // Page title already shown in admin breadcrumb (e.g. Mini Data · default / 数据源).
    const root = this.reset("");
    root.classList.add("mei-admin-explorer-root");
    root.setAttribute("aria-label", props.title || "资源");
    const resources = Array.isArray(props.resources)
      ? props.resources
      : Array.isArray(this._resources)
        ? this._resources
        : [];
    this._resources = resources;

    const filtered = this.filteredResources(props);

    const selectedStillVisible = filtered.some(
      (resource) => String(resource.id || "") === this._selectedId,
    );
    if (!filtered.length) {
      if (this._selectedId) {
        this._selectedId = "";
        this.syncExplorerUrl();
      }
    } else if (!this._selectedId || !selectedStillVisible) {
      this._selectedId = String(filtered[0].id || "");
      this.syncExplorerUrl();
    }

    const explorer = element("div", { className: "mei-admin-explorer" });
    explorer.style.setProperty("--nav-width", `${this._navWidthPct || readStoredNavWidth()}%`);
    const nav = element("div", { className: "mei-admin-explorer-nav" });
    const splitter = element("div", { className: "mei-admin-explorer-splitter" });
    splitter.setAttribute("role", "separator");
    splitter.setAttribute("aria-orientation", "vertical");
    splitter.setAttribute("aria-label", "调整左右宽度");
    splitter.tabIndex = 0;
    const detail = element("div", { className: "mei-admin-explorer-detail" });

    const toolbar = element("div", { className: "mei-admin-explorer-toolbar" });
    const search = element("input", { className: "mei-admin-explorer-search" });
    search.type = "search";
    search.placeholder =
      props.search_placeholder || props.searchPlaceholder || "搜索资源或文件名";
    search.value = this._query || "";
    search.setAttribute("aria-label", search.placeholder);
    search.addEventListener("input", (event) => {
      if (event.isComposing) return;
      this.applySearch(search.value, props, resources);
    });
    search.addEventListener("compositionend", () => {
      this.applySearch(search.value, props, resources);
    });
    toolbar.append(search);

    const viewToggle = element("div", { className: "mei-admin-view-toggle" });
    viewToggle.setAttribute("aria-label", "资源展示方式");
    [
      ["card", "卡片"],
      ["list", "列表"],
    ].forEach(([mode, label]) => {
      const button = element("button", {
        text: label,
        type: "button",
      });
      button.setAttribute("aria-pressed", String(this._viewMode === mode));
      button.addEventListener("click", () => {
        this._viewMode = mode;
        this.syncExplorerUrl();
        this.render({ ...props, resources });
      });
      viewToggle.append(button);
    });
    toolbar.append(viewToggle);
    toolbar.append(
      buildExplorerSortSelect({
        fields: collectionSortFields(resources),
        value: this._sort || { field: "title", dir: "asc" },
        ariaLabel: "资源排序",
        onChange: (sort) => this.applySort(sort, props),
      }),
    );
    nav.append(toolbar);

    const navScroll = element("div", { className: "mei-admin-explorer-scroll" });
    navScroll.append(this.buildResourceCollection(filtered, props));
    nav.append(navScroll);

    const detailScroll = element("div", { className: "mei-admin-explorer-scroll" });
    const selectedResource =
      filtered.find((resource) => String(resource.id || "") === this._selectedId) || null;
    if (selectedResource) {
      const assetSlot = document.createElement("mei-admin-asset-slot");
      assetSlot.setAttribute(
        "data-props",
        JSON.stringify({
          title: selectedResource.title || selectedResource.id,
          slot_id: selectedResource.id,
          accept: selectedResource.accept,
          hint: selectedResource.hint || "",
          list_provider: selectedResource.list_provider || selectedResource.listProvider,
          replace_provider: selectedResource.replace_provider || selectedResource.replaceProvider,
          slot: selectedResource.slot,
          embedded: true,
        }),
      );
      detailScroll.append(assetSlot);
    } else {
      detailScroll.append(
        element("div", {
          className: "mei-admin-explorer-detail-empty",
          text: "选择左侧资源以编辑",
        }),
      );
    }
    detail.append(detailScroll);

    this.bindSplitter(splitter, explorer);
    explorer.append(nav, splitter, detail);
    root.append(explorer);
    const scheduleFit = () => this.fitExplorerHeight();
    queueMicrotask(scheduleFit);
    requestAnimationFrame(scheduleFit);
    setTimeout(scheduleFit, 50);
    setTimeout(scheduleFit, 250);
  }
}

class DataGrid extends AdminBrick {
  async hydrate(props) {
    const refs = [props.rows_provider, ...(props.slot_providers || [])].filter(providerRefId);
    if (!refs.length) return;
    const payloads = await Promise.all(refs.map((reference) => readProvider(reference)));
    const rowsById = new Map();
    payloads
      .flatMap((payload) => payload.slots || payload.rows || [])
      .forEach((row, index) => {
        const normalized = normalizeSlotRow(row, index);
        rowsById.set(normalized.slot_id, normalized);
      });
    const rows = [...rowsById.values()];
    this.render({ ...props, rows });
  }

  render(props) {
    const root = this.reset(props.title || "Data");
    const rows = Array.isArray(props.rows) ? props.rows : [];
    const columns = Array.isArray(props.columns)
      ? props.columns
      : ["slot_id", "status", "kind", "active_path", "file_count", "size_bytes", "modified_ms"];
    const table = element("table", { className: "mei-admin-data-grid" });
    const colgroup = document.createElement("colgroup");
    columns.forEach((column) => {
      const col = document.createElement("col");
      col.className = `col-${column}`;
      colgroup.append(col);
    });
    table.append(colgroup);
    const head = element("thead");
    const headRow = element("tr");
    columns.forEach((column) =>
      headRow.append(element("th", { text: COLUMN_LABELS[column] || column })),
    );
    head.append(headRow);
    table.append(head);
    const body = element("tbody");
    rows.forEach((row) => {
      const tr = element("tr");
      columns.forEach((column) => {
        const raw = row?.[column] ?? "";
        if (column === "status") {
          const td = document.createElement("td");
          const chip = element("span", {
            className: "mei-admin-chip",
            text: statusLabel(raw),
          });
          chip.dataset.tone = String(raw || "").toLowerCase();
          td.append(chip);
          tr.append(td);
          return;
        }
        if (column === "active_path") {
          const td = element("td", {
            className: "mei-admin-cell-path",
            text: truncatePath(raw, 56),
          });
          td.title = String(raw || "");
          tr.append(td);
          return;
        }
        if (column === "size_bytes") {
          tr.append(element("td", { text: formatBytes(raw) }));
          return;
        }
        if (column === "modified_ms") {
          const ms = Number(raw);
          const text = Number.isFinite(ms) && ms > 0 ? new Date(ms).toLocaleString() : "—";
          tr.append(element("td", { text }));
          return;
        }
        tr.append(element("td", { text: raw === "" || raw == null ? "—" : raw }));
      });
      body.append(tr);
    });
    table.append(body);
    root.append(table);
    if (!rows.length) root.append(element("p", { className: "mei-admin-status", text: "暂无数据" }));
  }
}

class AssetSlot extends AdminBrick {
  async hydrate(props) {
    const listRef = props.list_provider || props.listProvider || props.replace_provider || props.replaceProvider;
    if (!providerRefId(listRef)) {
      this.render(props);
      return;
    }
    const response = await readProvider(listRef);
    const slots = response.slots || [];
    const slotId = String(props.slot_id || props.slotId || "").trim();
    let slot = null;
    if (slotId) {
      slot = slots.find((entry) => entry.slotId === slotId || entry.slot_id === slotId) || null;
    } else if (slots.length === 1) {
      slot = slots[0];
    }
    this.render({ ...props, slot, _slotMiss: Boolean(slotId) && !slot });
  }

  render(props) {
    const root = this.reset(props.title || props.slot_id || props.slotId || "Asset");
    if (props.embedded) root.classList.add("is-embedded");
    const slot = props.slot || {};
    const files = Array.isArray(slot.files) ? slot.files : [];
    const activePath = slot.activePath || slot.active_path || "";
    const accept = String(props.accept || ".xlsx,.xls,.csv").trim() || ".xlsx,.xls,.csv";
    const hint = String(props.hint || "").trim();
    if (hint) {
      root.append(element("p", { className: "mei-admin-hint", text: hint }));
    }

    if (props._slotMiss) {
      const miss = element("p", {
        className: "mei-admin-status",
        text: `未找到槽位 ${props.slot_id || props.slotId}，请检查 list_provider 与 slot_id 是否一致`,
      });
      miss.dataset.tone = "error";
      root.append(miss);
    }

    const pathBar = element("div", { className: "mei-admin-path-bar" });
    pathBar.append(element("span", { text: "当前" }));
    pathBar.append(document.createTextNode(activePath || "（尚未设置）"));
    pathBar.title = activePath || "";
    root.append(pathBar);

    const status = element("p", { className: "mei-admin-status" });
    root.append(status);

    const writeRef =
      props.replace_provider || props.replaceProvider || props.list_provider || props.listProvider;
    const canWrite = providerRefId(writeRef);

    const renderFileRow = (file, { current }) => {
      const name = file.name || "";
      const path = file.path || name;
      const row = element("div", {
        className: current ? "mei-admin-file-row is-current" : "mei-admin-file-row is-history",
      });
      const meta = element("div", { className: "mei-admin-file-meta" });
      const title = element("strong");
      title.append(element("span", { className: "mei-admin-file-name", text: name }));
      if (current) {
        title.append(element("span", { className: "mei-admin-badge", text: "当前" }));
      }
      meta.append(title);
      const subBits = [path];
      if (file.sizeBytes != null || file.size_bytes != null) {
        subBits.push(formatBytes(file.sizeBytes ?? file.size_bytes));
      }
      meta.append(element("span", { className: "mei-admin-file-sub", text: subBits.join(" · ") }));
      row.append(meta);

      const actions = element("div", { className: "mei-admin-file-actions" });
      if (canWrite) {
        if (!current) {
          const applyBtn = element("button", { text: "应用为当前", type: "button" });
          applyBtn.addEventListener("click", async () => {
            try {
              applyBtn.disabled = true;
              status.textContent = `正在应用 ${name}…`;
              const response = await applyAssetCurrent(writeRef, name);
              status.textContent = `已设为当前：${response.slot?.activePath || name}`;
              status.dataset.tone = "ok";
              await this.hydrate(props);
            } catch (error) {
              status.textContent = error.message || String(error);
              status.dataset.tone = "error";
            } finally {
              applyBtn.disabled = false;
            }
          });
          actions.append(applyBtn);
        }
        const downloadBtn = element("button", {
          text: "下载",
          type: "button",
          className: "mei-admin-btn-secondary",
        });
        downloadBtn.addEventListener("click", async () => {
          try {
            downloadBtn.disabled = true;
            status.textContent = `正在下载 ${name}…`;
            await downloadAssetFile(writeRef, name);
            status.textContent = `已下载 ${name}`;
            status.dataset.tone = "ok";
          } catch (error) {
            status.textContent = error.message || String(error);
            status.dataset.tone = "error";
          } finally {
            downloadBtn.disabled = false;
          }
        });
        actions.append(downloadBtn);
        if (!current) {
          const deleteBtn = element("button", {
            text: "删除",
            type: "button",
            className: "mei-admin-btn-danger",
          });
          deleteBtn.addEventListener("click", async () => {
            try {
              deleteBtn.disabled = true;
              status.textContent = `正在删除 ${name}…`;
              await deleteAssetFile(writeRef, name);
              status.textContent = `已删除 ${name}`;
              status.dataset.tone = "ok";
              await this.hydrate(props);
            } catch (error) {
              status.textContent = error.message || String(error);
              status.dataset.tone = "error";
            } finally {
              deleteBtn.disabled = false;
            }
          });
          actions.append(deleteBtn);
        }
      }
      row.append(actions);
      return row;
    };

    const currentFiles = files.filter((file) => file.isCurrent || file.is_current);
    const historyFiles = files.filter((file) => !(file.isCurrent || file.is_current));
    const list = element("div", { className: "mei-admin-file-list" });
    if (currentFiles.length) {
      const section = element("div", { className: "mei-admin-file-section" });
      section.append(element("p", { className: "mei-admin-file-section-title", text: "当前文件" }));
      currentFiles.forEach((file) => section.append(renderFileRow(file, { current: true })));
      list.append(section);
    }
    if (historyFiles.length) {
      const section = element("div", { className: "mei-admin-file-section" });
      section.append(element("p", { className: "mei-admin-file-section-title", text: "历史文件" }));
      historyFiles.forEach((file) => section.append(renderFileRow(file, { current: false })));
      list.append(section);
    }
    if (!files.length) {
      list.append(element("p", { className: "mei-admin-status", text: "暂无文件" }));
    }
    root.append(list);

    const upload = element("div", { className: "mei-admin-upload" });
    upload.append(element("label", { text: `上传新文件（${accept}）` }));
    const input = element("input");
    input.type = "file";
    input.accept = accept;
    input.addEventListener("change", async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        input.disabled = true;
        status.textContent = `正在上传 ${file.name}…`;
        const response = canWrite ? await replaceAsset(writeRef, file) : null;
        const uploaded =
          response?.slot?.files?.find((entry) => entry.name === file.name)?.name || file.name;
        status.textContent = `${uploaded} 已上传`;
        status.dataset.tone = "ok";
        this.dispatchEvent(
          new CustomEvent("mei:admin-asset-selected", {
            bubbles: true,
            composed: true,
            detail: { slotId: props.slot_id, file, response },
          }),
        );
        await this.hydrate(props);
      } catch (error) {
        status.textContent = error.message || String(error);
        status.dataset.tone = "error";
        console.error("[admin.asset-slot] upload failed", error);
      } finally {
        input.disabled = false;
        input.value = "";
      }
    });
    upload.append(input);
    root.append(upload);
    const surface = element("div", { className: "mei-admin-form-surface" });
    while (root.firstChild) surface.append(root.firstChild);
    root.append(surface);
  }
}

class ActionStrip extends AdminBrick {
  render(props) {
    const root = this.reset();
    root.classList.add("mei-admin-action-strip");
    const status = element("p", { className: "mei-admin-status" });
    (Array.isArray(props.actions) ? props.actions : []).forEach((action) => {
      const button = element("button", { text: action.label || action.id, type: "button" });
      button.dataset.action = String(action.id || "");
      button.addEventListener("click", async () => {
        if (
          (action.danger === true || action.danger === "danger") &&
          !window.confirm(action.confirm || `确定执行“${action.label || action.id}”？`)
        ) {
          return;
        }
        this.dispatchEvent(
          new CustomEvent("mei:admin-action", {
            bubbles: true,
            composed: true,
            detail: { action: action.id },
          }),
        );
        if (!providerRefId(action.provider)) return;
        try {
          button.disabled = true;
          status.textContent = "正在执行…";
          const response = await invokeProviderAction(action.provider, action.payload || {});
          status.textContent = response.message || "动作已完成";
          status.dataset.tone = "ok";
          this.dispatchEvent(
            new CustomEvent("mei:admin-action-complete", {
              bubbles: true,
              composed: true,
              detail: { action: action.id, response },
            }),
          );
        } catch (error) {
          status.textContent = error.message || String(error);
          status.dataset.tone = "error";
        } finally {
          button.disabled = false;
        }
      });
      root.append(button);
    });
    root.append(status);
  }
}

class JobStatus extends AdminBrick {
  async hydrate(props) {
    if (!providerRefId(props.status_provider) || !this._jobId) {
      this.render(props);
      return;
    }
    const response = await readProvider(props.status_provider, { jobId: this._jobId });
    const job = response.job || {};
    this.render({
      ...props,
      status: job.status || "idle",
      message: job.message || "",
    });
    if (job.status === "running" || job.status === "Running") {
      window.setTimeout(() => {
        void this.hydrate(props);
      }, 1200);
    }
  }

  render(props) {
    const root = this.reset(props.title || "Job");
    const status = props.status || "idle";
    root.dataset.status = String(status);
    root.append(element("strong", { text: status === "idle" ? "尚未启动" : status }));
    if (props.message) root.append(element("p", { text: props.message }));
  }
}

class WorkspaceShare extends AdminBrick {
  async hydrate(props) {
    if (this._sharePath == null) {
      const query = new URLSearchParams(window.location.search);
      this._sharePath = query.get("path") || "";
      this._shareQuery = query.get("q") || "";
      this._shareView = query.get("view") || "card";
      this._shareSort = parseExplorerSort(query.get("sort"), "name");
      this._shareSelected = query.get("sel") || "";
    }
    if (!this._shareSort) this._shareSort = { field: "name", dir: "asc" };
    const query = new URLSearchParams();
    if (this._sharePath) query.set("path", this._sharePath);
    const payload = await workspaceShareRequest(`/api/workspace/share?${query.toString()}`);
    this._shareEntries = payload.entries || [];
    this._shareDirectories = payload.directories || [];
    this.render(props);
  }

  syncShareUrl() {
    const url = new URL(window.location.href);
    [
      ["path", this._sharePath],
      ["q", this._shareQuery],
      ["view", this._shareView],
      ["sort", formatExplorerSort(this._shareSort || { field: "name", dir: "asc" })],
      ["sel", this._shareSelected],
    ].forEach(([key, value]) => {
      if (value) url.searchParams.set(key, value);
      else url.searchParams.delete(key);
    });
    window.history.replaceState(window.history.state, "", url);
  }

  navigate(path, props) {
    this._sharePath = String(path || "");
    this._shareSelected = "";
    this.syncShareUrl();
    void this.hydrate(props).catch((error) => this.showError(error));
  }

  async mutate(path, options, props, success) {
    this._shareStatus = "正在处理…";
    this._shareStatusError = false;
    this.paintShareStatus();
    try {
      await workspaceShareRequest(path, options);
      this._shareStatus = success;
      this._shareStatusError = false;
      await this.hydrate(props);
    } catch (error) {
      this._shareStatus = error.message || String(error);
      this._shareStatusError = true;
      this.paintShareStatus();
    }
  }

  filteredShareEntries() {
    const needle = String(this._shareQuery || "").trim().toLocaleLowerCase();
    const filtered = (this._shareEntries || []).filter((entry) =>
      String(entry.name || "").toLocaleLowerCase().includes(needle),
    );
    return sortShareEntries(filtered, this._shareSort || { field: "name", dir: "asc" });
  }

  ensureShareSelection(entries) {
    if (
      this._shareSelected &&
      !entries.some((entry) => String(entry.path || "") === this._shareSelected)
    ) {
      this._shareSelected = entries[0] ? String(entries[0].path || "") : "";
    } else if (!this._shareSelected && entries.length) {
      this._shareSelected = String(entries[0].path || "");
    }
  }

  applyShareSort(sort, props) {
    this._shareSort = sort || { field: "name", dir: "asc" };
    this.syncShareUrl();
    this.paintShareNavList(props);
    this.paintShareSelection();
  }

  paintShareStatus() {
    const status = this.querySelector("[data-share-status]");
    if (!status) return;
    status.textContent = this._shareStatus || `当前目录：${this._sharePath || "/"}`;
    if (this._shareStatusError) status.dataset.tone = "error";
    else delete status.dataset.tone;
  }

  paintShareNavList(props) {
    const navScroll = this.querySelector(".mei-admin-explorer-nav > .mei-admin-explorer-scroll");
    if (!navScroll) {
      this.render(props);
      return;
    }
    const entries = this.filteredShareEntries();
    this.ensureShareSelection(entries);
    this.syncShareUrl();
    navScroll.replaceChildren(this.buildShareCollection(entries, props));
  }

  buildShareCollection(entries, props) {
    const collection = element("div", { className: "mei-admin-resource-collection" });
    collection.dataset.view = this._shareView === "list" ? "list" : "card";
    collection.setAttribute("role", "listbox");
    collection.setAttribute("aria-label", "资料清单");
    const needle = String(this._shareQuery || "").trim().toLocaleLowerCase();
    entries.forEach((entry) => {
      const selected = this._shareSelected === entry.path;
      const card = element("article", {
        className: selected ? "mei-admin-resource-card is-selected" : "mei-admin-resource-card",
      });
      card.dataset.view = collection.dataset.view;
      card.dataset.sharePath = entry.path;
      card.setAttribute("role", "option");
      card.setAttribute("aria-selected", String(selected));
      card.tabIndex = 0;
      card.addEventListener("click", () => {
        if (this._shareSelected === entry.path) return;
        this._shareSelected = entry.path;
        this.syncShareUrl();
        this.paintShareSelection();
        this.paintShareDetail(props);
      });
      const head = element("div", { className: "mei-admin-resource-card-head" });
      const title = element("div", { className: "mei-admin-resource-card-title" });
      title.append(element("h3", { text: entry.name }));
      head.append(title);
      head.append(buildFileKindBadge(entry));
      card.append(head);
      card.append(
        element("p", {
          className: "mei-admin-resource-current",
          text: entry.isDir
            ? "文件夹"
            : `${formatBytes(entry.sizeBytes)} · ${
                entry.modifiedMs ? new Date(entry.modifiedMs).toLocaleString() : "—"
              }`,
        }),
      );
      collection.append(card);
    });
    if (!entries.length) {
      collection.append(
        element("div", {
          className: "mei-admin-explorer-empty",
          text: needle ? "没有匹配的文件或文件夹" : "当前文件夹为空",
        }),
      );
    }
    return collection;
  }

  buildShareBreadcrumb(props) {
    const breadcrumb = element("div", { className: "mei-workspace-share-breadcrumb" });
    const rootCrumb = element("button", { text: "资料交换", type: "button" });
    rootCrumb.addEventListener("click", () => this.navigate("", props));
    breadcrumb.append(rootCrumb);
    let accumulated = "";
    String(this._sharePath || "")
      .split("/")
      .filter(Boolean)
      .forEach((part) => {
        breadcrumb.append(document.createTextNode("/"));
        accumulated = accumulated ? `${accumulated}/${part}` : part;
        const path = accumulated;
        const crumb = element("button", { text: part, type: "button" });
        crumb.addEventListener("click", () => this.navigate(path, props));
        breadcrumb.append(crumb);
      });
    return breadcrumb;
  }

  buildShareUploadPanel(props) {
    const caps = props.capabilities || {};
    const upload = element("div", { className: "mei-workspace-share-upload-panel" });
    upload.dataset.shareUploadPanel = "true";
    upload.append(this.buildShareBreadcrumb(props));
    const rows = element("div", { className: "mei-workspace-share-upload-rows" });
    const row1 = element("div", { className: "mei-workspace-share-upload-row" });
    if (caps.upload !== false) {
      const input = element("input");
      input.type = "file";
      input.setAttribute("aria-label", "上传新文件");
      input.addEventListener("change", async () => {
        const file = input.files?.[0];
        if (!file) return;
        input.disabled = true;
        try {
          await uploadWorkspaceShareFile(file, this._sharePath, (progress) => {
            this._shareStatus = `正在上传 ${file.name}：${Math.round(progress * 100)}%`;
            this._shareStatusError = false;
            this.paintShareStatus();
          });
          this._shareStatus = `已上传 ${file.name}`;
          this._shareStatusError = false;
          const uploadedPath = this._sharePath ? `${this._sharePath}/${file.name}` : file.name;
          this._shareSelected = uploadedPath;
          await this.hydrate(props);
        } catch (error) {
          this._shareStatus = error.message || String(error);
          this._shareStatusError = true;
          this.paintShareStatus();
        } finally {
          input.disabled = false;
          input.value = "";
        }
      });
      row1.append(input);
    }
    rows.append(row1);
    const row2 = element("div", { className: "mei-workspace-share-upload-row" });
    if (caps.upload !== false) {
      const mkdir = element("button", {
        text: "新建文件夹",
        type: "button",
        className: "mei-admin-btn-secondary",
      });
      mkdir.addEventListener("click", () => {
        const name = window.prompt("文件夹名称");
        if (!name?.trim()) return;
        const path = this._sharePath ? `${this._sharePath}/${name.trim()}` : name.trim();
        void this.mutate(
          "/api/workspace/share/dir",
          {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ path, idempotency_key: idempotencyKey() }),
          },
          props,
          `已创建 ${path}`,
        );
      });
      row2.append(mkdir);
    }
    const status = element("p", {
      className: "mei-admin-status",
      text: this._shareStatus || `当前目录：${this._sharePath || "/"}`,
    });
    status.dataset.shareStatus = "true";
    if (this._shareStatusError) status.dataset.tone = "error";
    row2.append(status);
    rows.append(row2);
    upload.append(rows);
    return upload;
  }

  buildShareSelectionPanel(selected, props) {
    const caps = props.capabilities || {};
    const panel = element("div", { className: "mei-workspace-share-selection-panel" });
    panel.dataset.shareSelectionPanel = "true";
    if (!selected) {
      panel.append(
        element("div", {
          className: "mei-admin-explorer-detail-empty",
          text: "选择左侧文件或文件夹以操作",
        }),
      );
      return panel;
    }
    const titleRow = element("div", { className: "mei-workspace-share-selection-title" });
    titleRow.append(buildFileKindBadge(selected));
    titleRow.append(element("h2", { text: selected.name }));
    panel.append(titleRow);
    const kind = resolveFileKind(selected);
    const typeLabel = fileKindChipLabel(kind, selected.name);
    panel.append(
      element("p", {
        className: "mei-admin-hint",
        text: selected.isDir
          ? `类型：文件夹`
          : `类型：${typeLabel} · ${formatBytes(selected.sizeBytes)} · ${
              selected.modifiedMs ? new Date(selected.modifiedMs).toLocaleString() : "—"
            }`,
      }),
    );
    const entryActions = element("div", { className: "mei-workspace-share-entry-actions" });
    if (selected.isDir) {
      const open = element("button", { text: "打开", type: "button" });
      open.addEventListener("click", () => {
        this._shareSelected = "";
        this.navigate(selected.path, props);
      });
      entryActions.append(open);
    } else {
      const download = element("button", { text: "下载", type: "button" });
      download.addEventListener("click", () =>
        downloadWorkspaceShareFile(selected.path, selected.revision),
      );
      entryActions.append(download);
    }
    if (caps.organize !== false) {
      const rename = element("button", { text: "重命名", type: "button" });
      rename.addEventListener("click", () => {
        const nextName = window.prompt("新名称", selected.name);
        if (!nextName?.trim() || nextName.trim() === selected.name) return;
        const parent = selected.path.includes("/")
          ? selected.path.slice(0, selected.path.lastIndexOf("/"))
          : "";
        const toPath = parent ? `${parent}/${nextName.trim()}` : nextName.trim();
        this._shareSelected = toPath;
        void this.mutate(
          "/api/workspace/share/rename",
          {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({
              from_path: selected.path,
              to_path: toPath,
              expected_revision: selected.revision,
              idempotency_key: idempotencyKey(),
            }),
          },
          props,
          `已重命名为 ${toPath}`,
        );
      });
      entryActions.append(rename);
      const move = element("button", { text: "移动", type: "button" });
      move.addEventListener("click", () => {
        const toDir = window.prompt("目标文件夹（留空为根目录）", this._sharePath || "");
        if (toDir == null) return;
        void this.mutate(
          "/api/workspace/share/move",
          {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({
              from_path: selected.path,
              to_dir: toDir.trim() || null,
              expected_revision: selected.revision,
              idempotency_key: idempotencyKey(),
            }),
          },
          props,
          `已移动 ${selected.name}`,
        );
      });
      entryActions.append(move);
    }
    if (caps.delete !== false) {
      const remove = element("button", { text: "删除", type: "button" });
      remove.addEventListener("click", () => {
        if (!window.confirm(`确定删除 ${selected.name}？`)) return;
        void this.mutate(
          `/api/workspace/share?path=${encodeURIComponent(
            selected.path,
          )}&expected_revision=${encodeURIComponent(
            selected.revision,
          )}&idempotency_key=${encodeURIComponent(idempotencyKey())}`,
          { method: "DELETE" },
          props,
          `已删除 ${selected.name}`,
        );
      });
      entryActions.append(remove);
    }
    panel.append(entryActions);
    return panel;
  }

  render(props) {
    const root = this.reset();
    root.classList.add("mei-admin-explorer-root", "is-embedded");
    if (this._navWidthPct == null) this._navWidthPct = readStoredNavWidth();
    if (!this._shareSort) this._shareSort = { field: "name", dir: "asc" };

    const explorer = element("div", { className: "mei-admin-explorer mei-workspace-share" });
    explorer.style.setProperty("--nav-width", `${this._navWidthPct}%`);
    const navPane = element("div", { className: "mei-admin-explorer-nav" });
    const splitter = element("div", { className: "mei-admin-explorer-splitter" });
    splitter.setAttribute("role", "separator");
    splitter.setAttribute("aria-orientation", "vertical");
    splitter.setAttribute("aria-label", "调整左右宽度");
    splitter.tabIndex = 0;
    const detailPane = element("div", { className: "mei-admin-explorer-detail" });

    const folderNav = element("nav", { className: "mei-workspace-share-nav" });
    folderNav.setAttribute("aria-label", "资料文件夹");
    folderNav.append(element("h2", { text: "文件夹" }));
    const rootButton = element("button", { text: "全部资料", type: "button" });
    rootButton.setAttribute("aria-current", String(!this._sharePath));
    rootButton.addEventListener("click", () => {
      this._shareSelected = "";
      this.navigate("", props);
    });
    folderNav.append(rootButton);
    (this._shareDirectories || []).forEach((path) => {
      const button = element("button", { text: path, type: "button" });
      button.style.paddingLeft = `${8 + Math.max(0, path.split("/").length - 1) * 12}px`;
      button.title = path;
      button.setAttribute("aria-current", String(this._sharePath === path));
      button.addEventListener("click", () => {
        this._shareSelected = "";
        this.navigate(path, props);
      });
      folderNav.append(button);
    });
    navPane.append(folderNav);

    const toolbar = element("div", { className: "mei-admin-explorer-toolbar" });
    const search = element("input", { className: "mei-admin-explorer-search" });
    search.type = "search";
    search.placeholder = "搜索当前文件夹";
    search.value = this._shareQuery || "";
    search.addEventListener("input", () => {
      this._shareQuery = search.value;
      this.syncShareUrl();
      this.paintShareNavList(props);
      this.paintShareSelection();
      this.paintShareDetail(props);
      queueMicrotask(() => this.querySelector(".mei-admin-explorer-search")?.focus());
    });
    toolbar.append(search);
    const toggle = element("div", { className: "mei-admin-view-toggle" });
    [
      ["card", "卡片"],
      ["list", "列表"],
    ].forEach(([mode, label]) => {
      const button = element("button", { text: label, type: "button" });
      button.setAttribute("aria-pressed", String(this._shareView === mode));
      button.addEventListener("click", () => {
        this._shareView = mode;
        this.syncShareUrl();
        this.paintShareNavList(props);
        this.paintShareSelection();
      });
      toggle.append(button);
    });
    toolbar.append(toggle);
    toolbar.append(
      buildExplorerSortSelect({
        fields: ["name", "size", "time"],
        value: this._shareSort || { field: "name", dir: "asc" },
        ariaLabel: "资料排序",
        onChange: (sort) => this.applyShareSort(sort, props),
      }),
    );
    navPane.append(toolbar);

    const entries = this.filteredShareEntries();
    this.ensureShareSelection(entries);
    this.syncShareUrl();

    const navScroll = element("div", { className: "mei-admin-explorer-scroll" });
    navScroll.append(this.buildShareCollection(entries, props));
    navPane.append(navScroll);

    const detailScroll = element("div", { className: "mei-admin-explorer-scroll" });
    const panel = element("div", { className: "mei-workspace-share-detail-panel" });
    panel.append(this.buildShareUploadPanel(props));
    const selected =
      entries.find((entry) => String(entry.path || "") === this._shareSelected) || null;
    panel.append(this.buildShareSelectionPanel(selected, props));
    detailScroll.append(panel);
    detailPane.append(detailScroll);

    if (typeof this.bindSplitter === "function") {
      this.bindSplitter(splitter, explorer);
    } else if (typeof CollectionView.prototype.bindSplitter === "function") {
      CollectionView.prototype.bindSplitter.call(this, splitter, explorer);
    }
    explorer.append(navPane, splitter, detailPane);
    root.append(explorer);
    root.classList.add("mei-admin-explorer-root");
    queueMicrotask(() => this.fitExplorerHeight?.());
    requestAnimationFrame(() => this.fitExplorerHeight?.());
    setTimeout(() => this.fitExplorerHeight?.(), 50);
  }

  paintShareSelection() {
    this.querySelectorAll(".mei-admin-resource-card[data-share-path]").forEach((card) => {
      const selected = card.getAttribute("data-share-path") === this._shareSelected;
      card.classList.toggle("is-selected", selected);
      card.setAttribute("aria-selected", String(selected));
    });
  }

  paintShareDetail(props) {
    const selectionHost = this.querySelector("[data-share-selection-panel]");
    if (!selectionHost) {
      this.render(props);
      return;
    }
    const entries = this.filteredShareEntries();
    const selected =
      entries.find((entry) => String(entry.path || "") === this._shareSelected) || null;
    selectionHost.replaceWith(this.buildShareSelectionPanel(selected, props));
    const breadcrumbHost = this.querySelector("[data-share-upload-panel] .mei-workspace-share-breadcrumb");
    if (breadcrumbHost) {
      breadcrumbHost.replaceWith(this.buildShareBreadcrumb(props));
    }
    this.paintShareStatus();
  }
}

class Navigator extends AdminBrick {
  render(props) {
    const items = Array.isArray(props.items) ? props.items : [];
    if (!items.length || props.hidden === true) {
      this.hidden = true;
      this.replaceChildren();
      return;
    }
    this.hidden = false;
    const root = this.reset();
    const nav = element("nav");
    nav.setAttribute("aria-label", props.label || "资源分类");
    const appendItems = (parent, entries, depth = 0) => {
      entries.forEach((item) => {
        const link = element("a", { text: item.label || item.id });
        link.href = String(item.href || "#");
        link.style.paddingLeft = `${depth * 14}px`;
        if (item.active) link.setAttribute("aria-current", "page");
        parent.append(link);
        if (Array.isArray(item.children) && item.children.length) {
          appendItems(parent, item.children, depth + 1);
        }
      });
    };
    appendItems(nav, items);
    root.append(nav);
  }
}

[
  ["mei-admin-form-card", FormCard],
  ["mei-admin-grouped-form", GroupedForm],
  ["mei-admin-collection-view", CollectionView],
  ["mei-admin-data-grid", DataGrid],
  ["mei-admin-asset-slot", AssetSlot],
  ["mei-admin-action-strip", ActionStrip],
  ["mei-admin-job-status", JobStatus],
  ["mei-admin-navigator", Navigator],
  ["mei-workspace-share", WorkspaceShare],
].forEach(([tag, constructor]) => {
  if (!customElements.get(tag)) customElements.define(tag, constructor);
});

WorkspaceShare.prototype.fitExplorerHeight = CollectionView.prototype.fitExplorerHeight;
GroupedForm.prototype.fitExplorerHeight = CollectionView.prototype.fitExplorerHeight;
