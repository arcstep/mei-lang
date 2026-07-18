(() => {
  const root = document.getElementById("admin-asset-slot-root");
  if (!root) return;

  const appId = root.getAttribute("data-app-id") || "";
  const resourceId = root.getAttribute("data-resource-id") || "";
  let resourceSpec = null;
  try {
    resourceSpec = JSON.parse(root.getAttribute("data-admin-resource") || "null");
  } catch (_) {
    resourceSpec = null;
  }

  let slots = [];
  let selectedId = "";
  let busy = false;
  let lastJob = null;
  let statusMessage = "";
  let statusTone = "";

  function escapeHtml(value) {
    return String(value || "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;");
  }

  function selectedSlot() {
    return slots.find((s) => s.slotId === selectedId) || null;
  }

  function formatBytes(n) {
    const v = Number(n || 0);
    if (!Number.isFinite(v) || v <= 0) return "--";
    if (v < 1024) return `${v} B`;
    if (v < 1024 * 1024) return `${(v / 1024).toFixed(1)} KB`;
    return `${(v / (1024 * 1024)).toFixed(1)} MB`;
  }

  function jobChipHtml(job) {
    if (!job || typeof job !== "object") return "";
    const phase = escapeHtml(job.phase || job.status || "unknown");
    const message = escapeHtml(job.message || job.error || "");
    const id = escapeHtml(job.jobId || job.id || "");
    return `<div class="admin-kit-job-chip" data-asset-job-chip>
      <strong>${phase}</strong>
      ${id ? `<span class="mei-text-muted">${id}</span>` : ""}
      ${message ? `<span>${message}</span>` : ""}
    </div>
    <details class="admin-kit-job-details">
      <summary>任务明细</summary>
      <pre class="m-0 overflow-auto">${escapeHtml(JSON.stringify(job, null, 2))}</pre>
    </details>`;
  }

  function render() {
    const slot = selectedSlot();
    const list = slots
      .map((s) => {
        const active = s.slotId === selectedId ? " is-active" : "";
        return `<button type="button" class="admin-kit-nav-item admin-asset-slot-row${active}" data-slot-id="${escapeHtml(
          s.slotId,
        )}">
          <span class="admin-kit-nav-label">${escapeHtml(s.title || s.slotId)}</span>
          <span class="admin-kit-nav-meta">${escapeHtml(s.status)} · ${escapeHtml(s.path)}</span>
        </button>`;
      })
      .join("");

    const card = slot
      ? `<article class="admin-kit-card admin-asset-card">
          <header class="admin-kit-card-head">
            <h3 class="admin-kit-card-title">${escapeHtml(slot.title || slot.slotId)}</h3>
            <p class="admin-kit-card-desc">${escapeHtml(slot.path)} · ${escapeHtml(
              slot.kind,
            )} · ${escapeHtml(slot.status)} · ${formatBytes(slot.sizeBytes)}</p>
          </header>
          <label class="admin-kit-field">
            <span class="admin-kit-field-label">替换文件</span>
            <input class="admin-kit-field-input" type="file" data-asset-file ${busy ? "disabled" : ""} />
          </label>
          <div class="admin-kit-savebar-actions">
            <button type="button" class="admin-kit-btn admin-kit-btn-primary" data-asset-import ${
              busy ? "disabled" : ""
            }>导入并替换</button>
            <button type="button" class="admin-kit-btn admin-kit-btn-ghost" data-asset-refresh ${
              busy ? "disabled" : ""
            }>刷新</button>
          </div>
          <p class="admin-kit-status" data-asset-status data-tone="${escapeHtml(
            statusTone,
          )}">${escapeHtml(statusMessage)}</p>
          ${lastJob ? jobChipHtml(lastJob) : ""}
        </article>`
      : `<div class="admin-kit-card"><p class="admin-kit-card-desc">选择左侧槽位以查看与替换。</p></div>`;

    root.innerHTML = `<div class="admin-kit-layout admin-asset-slot-layout">
      <aside class="admin-kit-nav admin-asset-slot-list">
        <div class="admin-kit-nav-title">数据源槽位</div>
        <div class="admin-kit-nav-list">
          ${list || '<div class="admin-kit-card-desc">暂无槽位</div>'}
        </div>
      </aside>
      <div class="admin-kit-main admin-asset-slot-main">${card}</div>
    </div>`;

    root.querySelectorAll("[data-slot-id]").forEach((btn) => {
      btn.addEventListener("click", () => {
        selectedId = btn.getAttribute("data-slot-id") || "";
        lastJob = null;
        statusMessage = "";
        statusTone = "";
        render();
      });
    });
    const refreshBtn = root.querySelector("[data-asset-refresh]");
    if (refreshBtn) refreshBtn.addEventListener("click", () => loadSlots());
    const importBtn = root.querySelector("[data-asset-import]");
    if (importBtn) importBtn.addEventListener("click", () => runImport());
  }

  function setStatus(message, tone) {
    statusMessage = message || "";
    statusTone = tone || "";
    const el = root.querySelector("[data-asset-status]");
    if (!el) return;
    el.textContent = statusMessage;
    el.dataset.tone = statusTone;
  }

  async function loadSlots() {
    busy = true;
    render();
    try {
      const url = `/api/admin/providers/asset-slot?appId=${encodeURIComponent(
        appId,
      )}&resourceId=${encodeURIComponent(resourceId)}`;
      const resp = await fetch(url, { credentials: "same-origin" });
      const data = await resp.json().catch(() => ({}));
      if (!resp.ok) {
        throw new Error(data.message || `加载失败 (${resp.status})`);
      }
      slots = Array.isArray(data.slots) ? data.slots : [];
      if (!selectedId && slots[0]) selectedId = slots[0].slotId;
      if (selectedId && !slots.some((s) => s.slotId === selectedId)) {
        selectedId = slots[0] ? slots[0].slotId : "";
      }
      statusMessage = "";
      statusTone = "";
    } catch (err) {
      slots = [];
      root.innerHTML = `<div class="admin-kit-card admin-kit-card--danger">
        <div class="admin-kit-card-head">
          <h2 class="admin-kit-card-title">无法加载数据源</h2>
          <p class="admin-kit-card-desc">${escapeHtml(err.message || String(err))}</p>
        </div>
      </div>`;
      busy = false;
      return;
    }
    busy = false;
    render();
  }

  function bytesToHex(bytes) {
    return Array.from(bytes)
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
  }

  async function readFilePayload(file) {
    const name = file.name || "upload.bin";
    const lower = name.toLowerCase();
    if (lower.endsWith(".csv") || lower.endsWith(".txt") || lower.endsWith(".json")) {
      const content = await file.text();
      return { filename: name, content };
    }
    const buf = new Uint8Array(await file.arrayBuffer());
    return { filename: name, contentHex: bytesToHex(buf) };
  }

  async function runImport() {
    const slot = selectedSlot();
    if (!slot || busy) return;
    const input = root.querySelector("[data-asset-file]");
    const file = input && input.files && input.files[0];
    if (!file) {
      setStatus("请先选择要导入的文件", "warn");
      return;
    }
    busy = true;
    statusMessage = "正在导入…";
    statusTone = "";
    render();
    try {
      const payload = await readFilePayload(file);
      const resp = await fetch("/api/admin/providers/command-job", {
        method: "POST",
        credentials: "same-origin",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          appId,
          resourceId,
          action: "import",
          slotId: slot.slotId,
          filename: payload.filename,
          idempotencyKey: `import-${Date.now()}`,
          content: payload.content,
          contentHex: payload.contentHex,
        }),
      });
      const data = await resp.json().catch(() => ({}));
      if (!resp.ok) {
        throw new Error(data.message || `导入失败 (${resp.status})`);
      }
      lastJob = data.job || null;
      await loadSlots();
      setStatus("导入成功", "ok");
    } catch (err) {
      busy = false;
      statusMessage = err.message || String(err);
      statusTone = "err";
      render();
    }
  }

  if (!appId || !resourceId || !resourceSpec) {
    root.innerHTML = `<div class="admin-kit-card admin-kit-card--danger">
      <div class="admin-kit-card-head">
        <h2 class="admin-kit-card-title">缺少 Admin 资源投影</h2>
      </div>
    </div>`;
    return;
  }

  loadSlots();
})();
