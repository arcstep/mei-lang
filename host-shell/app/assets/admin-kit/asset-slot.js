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

  function render() {
    const slot = selectedSlot();
    const list = slots
      .map((s) => {
        const active = s.slotId === selectedId ? " is-active" : "";
        return `<button type="button" class="admin-asset-slot-row${active}" data-slot-id="${escapeHtml(
          s.slotId,
        )}">
          <strong>${escapeHtml(s.title || s.slotId)}</strong>
          <span class="mei-text-muted">${escapeHtml(s.status)} · ${escapeHtml(s.path)}</span>
        </button>`;
      })
      .join("");

    const card = slot
      ? `<article class="admin-asset-card rounded-lg border mei-border-default mei-surface-panel-muted p-3 flex flex-col gap-3">
          <header>
            <h3 class="mei-text-inverse mei-font-2 m-0">${escapeHtml(slot.title || slot.slotId)}</h3>
            <p class="mei-text-muted mei-font-1 m-0 mt-1">${escapeHtml(slot.path)} · ${escapeHtml(
              slot.kind,
            )} · ${escapeHtml(slot.status)} · ${formatBytes(slot.sizeBytes)}</p>
          </header>
          <label class="admin-asset-upload-field flex flex-col gap-1 mei-font-1">
            <span>替换文件</span>
            <input type="file" data-asset-file ${busy ? "disabled" : ""} />
          </label>
          <div class="flex flex-wrap gap-2">
            <button type="button" class="mei-host-shell__btn mei-host-shell__btn--primary" data-asset-import ${
              busy ? "disabled" : ""
            }>导入并替换</button>
            <button type="button" class="mei-host-shell__btn mei-host-shell__btn--ghost" data-asset-refresh ${
              busy ? "disabled" : ""
            }>刷新</button>
          </div>
          <p class="admin-asset-status mei-font-1 mei-text-body m-0" data-asset-status></p>
          ${
            lastJob
              ? `<pre class="admin-asset-job mei-font-1 overflow-auto m-0 p-2 rounded border mei-border-default">${escapeHtml(
                  JSON.stringify(lastJob, null, 2),
                )}</pre>`
              : ""
          }
        </article>`
      : `<div class="mei-text-muted mei-font-1">选择左侧槽位以查看与替换。</div>`;

    root.innerHTML = `<div class="admin-asset-slot-layout grid gap-3 min-h-0" style="grid-template-columns: minmax(220px, 280px) 1fr;">
      <aside class="admin-asset-slot-list flex flex-col gap-1 overflow-auto">
        <div class="mei-font-1 mei-text-muted mb-1">数据源槽位</div>
        ${list || '<div class="mei-text-muted">暂无槽位</div>'}
      </aside>
      <div class="admin-asset-slot-main min-w-0">${card}</div>
    </div>`;

    root.querySelectorAll("[data-slot-id]").forEach((btn) => {
      btn.addEventListener("click", () => {
        selectedId = btn.getAttribute("data-slot-id") || "";
        lastJob = null;
        render();
      });
    });
    const refreshBtn = root.querySelector("[data-asset-refresh]");
    if (refreshBtn) refreshBtn.addEventListener("click", () => loadSlots());
    const importBtn = root.querySelector("[data-asset-import]");
    if (importBtn) importBtn.addEventListener("click", () => runImport());
  }

  function setStatus(message, tone) {
    const el = root.querySelector("[data-asset-status]");
    if (!el) return;
    el.textContent = message || "";
    el.dataset.tone = tone || "";
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
      setStatus("");
    } catch (err) {
      slots = [];
      root.innerHTML = `<div class="admin-form-error rounded-lg border mei-border-danger px-3 py-2 mei-text-body">${escapeHtml(
        err.message || String(err),
      )}</div>`;
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
    render();
    setStatus("正在导入…");
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
      render();
      setStatus(err.message || String(err), "err");
    }
  }

  if (!appId || !resourceId || !resourceSpec) {
    root.innerHTML =
      '<div class="admin-form-error rounded-lg border mei-border-danger px-3 py-2 mei-text-body">缺少 Admin 资源投影</div>';
    return;
  }

  loadSlots();
})();
