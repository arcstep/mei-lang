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

function ensureAdminStyles() {
  if (document.getElementById("mei-admin-brick-styles")) return;
  const style = document.createElement("style");
  style.id = "mei-admin-brick-styles";
  style.textContent = `
    .mei-compose-document-host { padding: 0 24px 24px; }
    .mei-compose-document-host .preview-card { padding: 0; border: 0; background: transparent; overflow: visible; }
    .mei-compose-document-host .component-host { overflow: visible; }
    .mei-admin-entry-copy { max-width: 1120px; line-height: 1.7; color: var(--mei-color-text-body, #cbd5e1); }
    .mei-admin-entry-copy h2 { margin: 12px 0 6px; color: var(--mei-color-text-primary, #fff); font-size: 18px; }
    .mei-admin-brick { max-width: 1120px; margin: 12px 0; padding: 20px; border: 1px solid var(--mei-color-border-default, rgba(148,163,184,.24)); border-radius: 10px; background: rgba(15, 23, 42, .72); box-sizing: border-box; }
    .mei-admin-brick h2 { margin: 0 0 16px; color: var(--mei-color-text-primary, #fff); font-size: 20px; }
    .mei-admin-form-card { display: grid; gap: 14px; }
    .mei-admin-field { display: grid; gap: 6px; color: var(--mei-color-text-body, #cbd5e1); }
    .mei-admin-field input, .mei-admin-field textarea { width: 100%; padding: 9px 11px; color: var(--mei-color-text-primary, #fff); border: 1px solid var(--mei-color-input-border, #334155); border-radius: 6px; background: var(--mei-color-input-bg, rgba(15,23,42,.8)); box-sizing: border-box; }
    .mei-admin-field textarea { min-height: 120px; font-family: ui-monospace, monospace; }
    .mei-admin-brick button { justify-self: start; padding: 8px 14px; color: var(--mei-color-btn-primary-text, #041320); border: 0; border-radius: 6px; background: var(--mei-color-btn-primary-bg, #38bdf8); cursor: pointer; }
    .mei-admin-brick button:disabled { opacity: .55; cursor: wait; }
    .mei-admin-data-grid { width: 100%; border-collapse: collapse; }
    .mei-admin-data-grid th, .mei-admin-data-grid td { padding: 9px 10px; text-align: left; border-bottom: 1px solid var(--mei-color-table-row-border, rgba(148,163,184,.16)); }
    .mei-admin-data-grid th { color: var(--mei-color-text-muted, #94a3b8); }
    .mei-admin-action-strip { display: flex; flex-wrap: wrap; gap: 10px; }
    .mei-admin-status { margin: 10px 0 0; color: var(--mei-color-text-muted, #94a3b8); }
    .mei-admin-status[data-tone="error"] { color: var(--mei-color-status-error, #fca5a5); }
  `;
  document.head.appendChild(style);
}

class AdminBrick extends HTMLElement {
  static observedAttributes = ["data-props"];

  connectedCallback() {
    ensureAdminStyles();
    this.style.display = "block";
    this.style.width = "100%";
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
}

class FormCard extends AdminBrick {
  async hydrate(props) {
    if (!providerRefId(props.payload_provider)) return;
    const response = await readProvider(props.payload_provider);
    this._revision = Number(response.revision || 0);
    this.render({ ...props, payload: response.payload || {} });
  }

  render(props) {
    const root = this.reset(props.title || "Form");
    const form = element("form", { className: "mei-admin-form-card" });
    const payload = props.payload && typeof props.payload === "object" ? props.payload : {};
    const fields = Array.isArray(props.fields) && props.fields.length
      ? props.fields
      : Object.keys(payload);
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
      label.append(input);
      form.append(label);
    });
    const save = element("button", { text: props.submit_label || "保存", type: "submit" });
    form.append(save);
    const status = element("p", { className: "mei-admin-status" });
    form.append(status);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const detail = {};
      try {
        form.querySelectorAll("[name]").forEach((input) => {
          detail[input.name] =
            input.dataset.valueKind === "json" ? JSON.parse(input.value || "{}") : input.value;
        });
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
    root.append(form);
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
        const normalized = Object.fromEntries(
          Object.entries(row || {}).map(([key, value]) => [
            key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`),
            value,
          ]),
        );
        rowsById.set(normalized.slot_id || normalized.id || `row-${index}`, normalized);
      });
    const rows = [...rowsById.values()];
    this.render({ ...props, rows });
  }

  render(props) {
    const root = this.reset(props.title || "Data");
    const rows = Array.isArray(props.rows) ? props.rows : [];
    const columns = Array.isArray(props.columns)
      ? props.columns
      : [...new Set(rows.flatMap((row) => Object.keys(row || {})))];
    const table = element("table", { className: "mei-admin-data-grid" });
    const head = element("thead");
    const headRow = element("tr");
    columns.forEach((column) => headRow.append(element("th", { text: column })));
    head.append(headRow);
    table.append(head);
    const body = element("tbody");
    rows.forEach((row) => {
      const tr = element("tr");
      columns.forEach((column) => tr.append(element("td", { text: row?.[column] ?? "" })));
      body.append(tr);
    });
    table.append(body);
    root.append(table);
    if (!rows.length) root.append(element("p", { className: "mei-admin-status", text: "暂无数据" }));
  }
}

class AssetSlot extends AdminBrick {
  render(props) {
    const root = this.reset(props.title || props.slot_id || "Asset");
    const status = element("p", {
      className: "mei-admin-status",
      text: props.status || "等待选择文件",
    });
    root.append(status);
    const input = element("input");
    input.type = "file";
    input.addEventListener("change", async () => {
      const file = input.files?.[0];
      if (file) {
        try {
          input.disabled = true;
          status.textContent = `正在上传 ${file.name}…`;
          const response = providerRefId(props.replace_provider)
            ? await replaceAsset(props.replace_provider, file)
            : null;
          status.textContent = `${file.name} 已就绪`;
          this.dispatchEvent(
            new CustomEvent("mei:admin-asset-selected", {
              bubbles: true,
              composed: true,
              detail: { slotId: props.slot_id, file, response },
            }),
          );
        } catch (error) {
          status.textContent = error.message || String(error);
          status.dataset.tone = "error";
          console.error("[admin.asset-slot] upload failed", error);
        } finally {
          input.disabled = false;
        }
      }
    });
    root.append(input);
  }
}

class ActionStrip extends AdminBrick {
  render(props) {
    const root = this.reset();
    root.classList.add("mei-admin-action-strip");
    (Array.isArray(props.actions) ? props.actions : []).forEach((action) => {
      const button = element("button", { text: action.label || action.id, type: "button" });
      button.dataset.action = String(action.id || "");
      button.addEventListener("click", () => {
        this.dispatchEvent(
          new CustomEvent("mei:admin-action", {
            bubbles: true,
            composed: true,
            detail: { action: action.id },
          }),
        );
      });
      root.append(button);
    });
  }
}

class JobStatus extends AdminBrick {
  render(props) {
    const root = this.reset(props.title || "Job");
    const status = props.status || "idle";
    root.dataset.status = String(status);
    root.append(element("strong", { text: status === "idle" ? "尚未启动" : status }));
    if (props.message) root.append(element("p", { text: props.message }));
  }
}

class Navigator extends AdminBrick {
  render(props) {
    const root = this.reset();
    const nav = element("nav");
    (Array.isArray(props.items) ? props.items : []).forEach((item) => {
      const link = element("a", { text: item.label || item.id });
      link.href = String(item.href || "#");
      if (item.active) link.setAttribute("aria-current", "page");
      nav.append(link);
    });
    root.append(nav);
  }
}

[
  ["mei-admin-form-card", FormCard],
  ["mei-admin-data-grid", DataGrid],
  ["mei-admin-asset-slot", AssetSlot],
  ["mei-admin-action-strip", ActionStrip],
  ["mei-admin-job-status", JobStatus],
  ["mei-admin-navigator", Navigator],
].forEach(([tag, constructor]) => {
  if (!customElements.get(tag)) customElements.define(tag, constructor);
});
