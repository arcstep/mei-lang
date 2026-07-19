(() => {
  function resolveFormRoot() {
    const compose = document.getElementById("mei-admin-compose-root");
    const legacy = document.getElementById("admin-form-root");
    if (legacy && !compose) {
      console.error(
        "[admin-kit/form-card] #admin-form-root must mount under #mei-admin-compose-root"
      );
      return null;
    }
    if (!compose) return null;
    const root = compose.querySelector("#admin-form-root");
    if (!root) return null;
    return root;
  }

  const root = resolveFormRoot();
  if (!root) return;

  const appId = root.getAttribute("data-app-id") || "";
  const resourceId = root.getAttribute("data-resource-id") || "";
  let resourceSpec = null;
  try {
    resourceSpec = JSON.parse(root.getAttribute("data-admin-resource") || "null");
  } catch (_) {
    resourceSpec = null;
  }

  let revision = 0;
  let persistedRevision = 0;
  let effectiveRevision = 0;
  let applyPolicy = "hot";
  let baseline = {};
  let dirty = false;
  let saving = false;

  function fieldDefs() {
    const sections = (resourceSpec && resourceSpec.spec && resourceSpec.spec.sections) || [];
    const fields = [];
    for (const section of sections) {
      for (const field of section.fields || []) {
        fields.push({ ...field, sectionTitle: section.title || section.id });
      }
    }
    return fields;
  }

  function markDirty(next) {
    dirty = !!next;
    root.dataset.dirty = dirty ? "1" : "0";
    const saveBtn = root.querySelector("[data-admin-save]");
    if (saveBtn) saveBtn.disabled = saving || !dirty;
  }

  window.addEventListener("beforeunload", (event) => {
    if (!dirty) return;
    event.preventDefault();
    event.returnValue = "";
  });

  function escapeHtml(value) {
    return String(value)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function valueAtPath(value, path) {
    return String(path || "")
      .split(".")
      .filter(Boolean)
      .reduce((current, segment) => (current == null ? undefined : current[segment]), value);
  }

  function setValueAtPath(value, path, next) {
    const segments = String(path || "").split(".").filter(Boolean);
    if (!segments.length) return;
    let current = value;
    for (const segment of segments.slice(0, -1)) {
      if (!current[segment] || typeof current[segment] !== "object") current[segment] = {};
      current = current[segment];
    }
    current[segments[segments.length - 1]] = next;
  }

  function renderError(message) {
    root.innerHTML = `<div class="admin-kit-card admin-kit-card--danger">
      <div class="admin-kit-card-head">
        <h2 class="admin-kit-card-title">无法加载表单</h2>
        <p class="admin-kit-card-desc">${escapeHtml(message || "加载失败")}</p>
      </div>
    </div>`;
  }

  function renderForm(payload) {
    const fields = fieldDefs();
    if (!fields.length) {
      renderError("资源未声明可编辑字段，或资源未注册。");
      return;
    }
    const groups = new Map();
    for (const field of fields) {
      const key = field.sectionTitle || "表单";
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(field);
    }

    let sectionsHtml = "";
    for (const [title, group] of groups.entries()) {
      sectionsHtml += `<section class="admin-kit-section">
        <h2 class="admin-kit-section-title">${escapeHtml(title)}</h2>
        <div class="grid gap-3">`;
      for (const field of group) {
        const value = valueAtPath(payload, field.value_path || field.id) ?? "";
        const required = field.required ? "required" : "";
        const readonly = field.readonly ? "readonly disabled" : "";
        const control = field.control || "text";
        const label = escapeHtml(field.label || field.id);
        const name = escapeHtml(field.id);
        if (control === "textarea") {
          sectionsHtml += `<label class="admin-kit-field">
            <span class="admin-kit-field-label">${label}</span>
            <textarea class="admin-kit-field-input" name="${name}" ${required} ${readonly}>${escapeHtml(
              value,
            )}</textarea>
          </label>`;
        } else if (control === "boolean") {
          sectionsHtml += `<label class="admin-kit-field admin-kit-field--check">
            <input type="checkbox" name="${name}" ${value ? "checked" : ""} ${readonly}/>
            <span class="admin-kit-field-label">${label}</span>
          </label>`;
        } else if (control === "select") {
          const options = (field.options || [])
            .map(
              (option) =>
                `<option value="${escapeHtml(option.value)}" ${
                  String(option.value) === String(value) ? "selected" : ""
                }>${escapeHtml(option.label || option.value)}</option>`,
            )
            .join("");
          sectionsHtml += `<label class="admin-kit-field">
            <span class="admin-kit-field-label">${label}</span>
            <select class="admin-kit-field-input" name="${name}" ${required} ${readonly}>${options}</select>
          </label>`;
        } else {
          const type = control === "number" ? "number" : "text";
          sectionsHtml += `<label class="admin-kit-field">
            <span class="admin-kit-field-label">${label}</span>
            <input class="admin-kit-field-input" type="${type}" name="${name}" value="${escapeHtml(
              value,
            )}" ${required} ${readonly}/>
          </label>`;
        }
      }
      sectionsHtml += `</div></section>`;
    }

    root.innerHTML = `<form class="admin-kit-card admin-form-card" data-admin-form="1">
      <div class="admin-kit-card-head">
        <h2 class="admin-kit-card-title">编辑</h2>
        <p class="admin-kit-card-desc">修改后保存；未保存离开将提示确认。</p>
      </div>
      ${sectionsHtml}
      <div class="admin-kit-savebar">
        <div class="admin-kit-savebar-actions">
          <button type="submit" class="admin-kit-btn admin-kit-btn-primary" data-admin-save>保存</button>
          <button type="button" class="admin-kit-btn admin-kit-btn-ghost" data-admin-reset>重置</button>
          <span class="admin-kit-status" data-admin-status></span>
        </div>
        <span class="admin-kit-savebar-meta" data-admin-revision>持久 ${persistedRevision} · 生效 ${effectiveRevision} · ${escapeHtml(
          applyPolicy,
        )}</span>
      </div>
    </form>`;

    const form = root.querySelector("[data-admin-form]");
    form.addEventListener("input", () => markDirty(true));
    form.addEventListener("change", () => markDirty(true));
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      await saveForm(form);
    });
    root.querySelector("[data-admin-reset]")?.addEventListener("click", () => {
      renderForm(baseline);
      markDirty(false);
      setStatus("已重置");
    });
    markDirty(false);
  }

  function readPayload(form) {
    const data = JSON.parse(JSON.stringify(baseline || {}));
    const fields = fieldDefs();
    for (const field of fields) {
      const el = form.elements.namedItem(field.id);
      if (!el) continue;
      if (field.control === "boolean") {
        setValueAtPath(data, field.value_path || field.id, !!el.checked);
      } else if (field.control === "number") {
        setValueAtPath(
          data,
          field.value_path || field.id,
          el.value === "" ? null : Number(el.value),
        );
      } else {
        setValueAtPath(data, field.value_path || field.id, el.value);
      }
      const fieldValue = valueAtPath(data, field.value_path || field.id);
      if (
        field.required &&
        field.control !== "boolean" &&
        (fieldValue === "" || fieldValue == null)
      ) {
        throw new Error(`${field.label || field.id} 为必填`);
      }
    }
    return data;
  }

  function setStatus(text) {
    const status = root.querySelector("[data-admin-status]");
    if (status) status.textContent = text || "";
  }

  async function saveForm(form) {
    if (saving) return;
    let payload;
    try {
      payload = readPayload(form);
    } catch (err) {
      setStatus(err.message || "校验失败");
      return;
    }
    saving = true;
    markDirty(dirty);
    setStatus("保存中…");
    try {
      const resp = await fetch("/api/admin/providers/config-record", {
        method: "PUT",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          appId,
          resourceId,
          revision,
          idempotencyKey: `admin-${Date.now()}-${Math.random().toString(16).slice(2)}`,
          payload,
        }),
      });
      const body = await resp.json().catch(() => ({}));
      if (!resp.ok) {
        if (body.kind === "conflict") {
          setStatus(`冲突：当前 revision=${body.currentRevision ?? "?"}，请刷新后重试`);
        } else {
          setStatus(body.message || body.kind || `保存失败 (${resp.status})`);
        }
        return;
      }
      revision = body.revision ?? revision + 1;
      persistedRevision = body.persistedRevision ?? revision;
      effectiveRevision = body.effectiveRevision ?? persistedRevision;
      applyPolicy = body.applyPolicy || applyPolicy;
      baseline = body.payload || payload;
      renderForm(baseline);
      markDirty(false);
      setStatus(body.runtimeRestartRequired ? "已保存；需要重启 Runtime 后生效" : "已保存并生效");
    } catch (err) {
      setStatus(err.message || "网络错误");
    } finally {
      saving = false;
      markDirty(dirty);
    }
  }

  async function load() {
    root.innerHTML = `<div class="admin-kit-card"><p class="admin-kit-card-desc">加载中…</p></div>`;
    if (!appId || !resourceId) {
      renderError("缺少 app / resource 上下文");
      return;
    }
    if (!resourceSpec || resourceSpec === null) {
      renderError("资源未注册或无权访问");
      return;
    }
    try {
      const url = `/api/admin/providers/config-record?appId=${encodeURIComponent(appId)}&resourceId=${encodeURIComponent(resourceId)}`;
      const resp = await fetch(url);
      const body = await resp.json().catch(() => ({}));
      if (!resp.ok) {
        renderError(body.message || body.kind || `加载失败 (${resp.status})`);
        return;
      }
      revision = body.revision || 0;
      persistedRevision = body.persistedRevision ?? revision;
      effectiveRevision = body.effectiveRevision ?? persistedRevision;
      applyPolicy = body.applyPolicy || "hot";
      baseline = body.payload && typeof body.payload === "object" ? body.payload : {};
      if (body.spec) {
        resourceSpec = { ...resourceSpec, spec: body.spec };
      }
      renderForm(baseline);
    } catch (err) {
      renderError(err.message || "网络错误");
    }
  }

  load();
})();
