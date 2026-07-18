(() => {
  const root = document.getElementById("admin-form-root");
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
        const value = payload[field.id] ?? "";
        const required = field.required ? "required" : "";
        const control = field.control || "text";
        const label = escapeHtml(field.label || field.id);
        const name = escapeHtml(field.id);
        if (control === "textarea") {
          sectionsHtml += `<label class="admin-kit-field">
            <span class="admin-kit-field-label">${label}</span>
            <textarea class="admin-kit-field-input" name="${name}" ${required}>${escapeHtml(
              value,
            )}</textarea>
          </label>`;
        } else if (control === "boolean") {
          sectionsHtml += `<label class="admin-kit-field admin-kit-field--check">
            <input type="checkbox" name="${name}" ${value ? "checked" : ""}/>
            <span class="admin-kit-field-label">${label}</span>
          </label>`;
        } else {
          const type = control === "number" ? "number" : "text";
          sectionsHtml += `<label class="admin-kit-field">
            <span class="admin-kit-field-label">${label}</span>
            <input class="admin-kit-field-input" type="${type}" name="${name}" value="${escapeHtml(
              value,
            )}" ${required}/>
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
        <span class="admin-kit-savebar-meta" data-admin-revision>revision ${revision}</span>
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
    const data = {};
    const fields = fieldDefs();
    for (const field of fields) {
      const el = form.elements.namedItem(field.id);
      if (!el) continue;
      if (field.control === "boolean") {
        data[field.id] = !!el.checked;
      } else if (field.control === "number") {
        data[field.id] = el.value === "" ? null : Number(el.value);
      } else {
        data[field.id] = el.value;
      }
      if (
        field.required &&
        field.control !== "boolean" &&
        (data[field.id] === "" || data[field.id] == null)
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
      baseline = body.payload || payload;
      renderForm(baseline);
      markDirty(false);
      setStatus("已保存");
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
