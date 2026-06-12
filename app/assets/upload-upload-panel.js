(function initUploadPanel() {
  const root = document.getElementById("upload-panel-root");
  if (!root) return;

  const appId = root.dataset.appId || "";
  const selectedFile = root.dataset.selectedFile || "";
  const selectedDir = root.dataset.selectedDir || "";
  const selectedIsDir = root.dataset.selectedIsDir === "1";
  const uploadRoot = root.dataset.uploadRoot || "upload";
  const CHUNK_THRESHOLD_BYTES = 16 * 1024 * 1024;
  const CHUNK_SIZE_BYTES = 8 * 1024 * 1024;
  let isUploading = false;
  let queue = [];

  function escapeHtml(value) {
    return String(value || "")
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&#39;");
  }

  function formatBytes(bytes) {
    if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit += 1;
    }
    const digits = unit === 0 ? 0 : value >= 100 ? 0 : value >= 10 ? 1 : 1;
    return `${value.toFixed(digits)} ${units[unit]}`;
  }

  function queueKey(file) {
    return `${file.name}::${file.size}::${file.lastModified || 0}`;
  }

  function fileToken(name, isDir) {
    if (isDir) return { kind: "dir", label: "DIR" };
    const ext = name.includes(".")
      ? name.slice(name.lastIndexOf(".") + 1).toLowerCase()
      : "";
    if (["png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif"].includes(ext)) {
      return { kind: "image", label: "IMG" };
    }
    if (["mp4", "mov", "avi", "mkv", "webm", "m4v"].includes(ext)) {
      return { kind: "video", label: "VID" };
    }
    if (["mp3", "wav", "flac", "aac", "m4a"].includes(ext)) {
      return { kind: "audio", label: "AUD" };
    }
    if (["csv", "xlsx", "xls"].includes(ext)) {
      return { kind: "sheet", label: "CSV" };
    }
    if (["json", "jsonc", "yaml", "yml", "toml"].includes(ext)) {
      return { kind: "data", label: "JSON" };
    }
    if (["js", "jsx", "mjs", "cjs"].includes(ext)) {
      return { kind: "code", label: "JS" };
    }
    if (["ts", "tsx"].includes(ext)) {
      return { kind: "code", label: "TS" };
    }
    if (["css", "scss", "less"].includes(ext)) {
      return { kind: "code", label: "CSS" };
    }
    if (["md", "markdown", "txt"].includes(ext)) {
      return { kind: "doc", label: "TXT" };
    }
    if (["pdf"].includes(ext)) {
      return { kind: "doc", label: "PDF" };
    }
    if (["zip", "tar", "gz", "rar", "7z"].includes(ext)) {
      return { kind: "archive", label: "ZIP" };
    }
    return { kind: "file", label: "FILE" };
  }

  function currentDirLabel() {
    return selectedDir ? `${uploadRoot}/${selectedDir}` : uploadRoot;
  }

  root.innerHTML = `
    <div class="upload-panel-shell">
      <section class="upload-panel-card upload-panel-card--hero">
        <div class="upload-panel-hero">
          <div class="upload-panel-hero-copy">
            <div class="upload-panel-kicker">上传中心</div>
            <div class="upload-panel-title">批量上传与大文件分段上传</div>
            <div class="upload-panel-note">
              当前目标目录：<span class="upload-panel-dir">${escapeHtml(currentDirLabel())}</span>
            </div>
          </div>
          <div class="upload-panel-badges">
            <span class="upload-panel-badge">自动滚动清单</span>
            <span class="upload-panel-badge">拖拽上传</span>
            <span class="upload-panel-badge">>${formatBytes(CHUNK_THRESHOLD_BYTES)} 自动分段</span>
          </div>
        </div>
        <form id="upload-file-form" class="upload-form">
          <input id="upload-file-input" type="file" name="file" multiple hidden />
          <div
            id="upload-dropzone"
            class="upload-dropzone"
            tabindex="0"
            role="button"
            aria-label="拖拽文件到这里或点击选择文件"
          >
            <div class="upload-dropzone-icon" aria-hidden="true">UP</div>
            <div class="upload-dropzone-copy">
              <div class="upload-dropzone-title">拖拽文件到这里，或点击选择文件</div>
              <div class="upload-dropzone-note">支持多文件队列；较大的视频文件会自动走分段上传。</div>
            </div>
          </div>
          <div class="upload-form-actions">
            <button type="button" id="upload-pick-btn" class="upload-btn upload-btn--secondary">选择文件</button>
            <button type="submit" id="upload-submit-btn" class="upload-btn upload-btn--primary">开始上传</button>
          </div>
          <div id="upload-selection-summary" class="upload-selection-summary" hidden></div>
          <div id="upload-selected-list" class="upload-selected-list" hidden></div>
          <p id="upload-panel-status" class="upload-panel-status"></p>
        </form>
      </section>
    ${
      selectedFile && !selectedIsDir
        ? `
      <section class="upload-panel-card upload-panel-card--danger">
        <div class="upload-panel-danger-copy">
          <div class="upload-panel-danger-title">删除当前文件</div>
          <div class="upload-panel-danger-note">${escapeHtml(selectedFile)}</div>
        </div>
        <button type="button" id="upload-delete-btn" class="upload-btn upload-btn--danger">删除当前文件</button>
      </section>`
        : ""
    }
    </div>
  `;

  const statusEl = document.getElementById("upload-panel-status");
  const form = document.getElementById("upload-file-form");
  const input = document.getElementById("upload-file-input");
  const pickBtn = document.getElementById("upload-pick-btn");
  const submitBtn = document.getElementById("upload-submit-btn");
  const dropzone = document.getElementById("upload-dropzone");
  const selectedListEl = document.getElementById("upload-selected-list");
  const selectionSummaryEl = document.getElementById("upload-selection-summary");
  const deleteBtn = document.getElementById("upload-delete-btn");

  function setStatus(text, tone) {
    if (!statusEl) return;
    statusEl.textContent = text || "";
    statusEl.className = `upload-panel-status${
      tone ? ` upload-panel-status--${tone}` : ""
    }`;
  }

  function setUploading(nextValue) {
    isUploading = !!nextValue;
    if (pickBtn) pickBtn.disabled = isUploading;
    if (submitBtn) submitBtn.disabled = isUploading || queue.length === 0;
    if (input) input.disabled = isUploading;
  }

  function syncSelectionSummary() {
    if (!selectionSummaryEl) return;
    if (!queue.length) {
      selectionSummaryEl.hidden = true;
      selectionSummaryEl.innerHTML = "";
      return;
    }
    const totalBytes = queue.reduce((sum, item) => sum + item.file.size, 0);
    const chunkedCount = queue.filter(
      (item) => item.file.size >= CHUNK_THRESHOLD_BYTES,
    ).length;
    selectionSummaryEl.hidden = false;
    selectionSummaryEl.innerHTML = `
      <span class="upload-summary-chip">${queue.length} 个待上传文件</span>
      <span class="upload-summary-chip">${formatBytes(totalBytes)}</span>
      <span class="upload-summary-chip">${chunkedCount} 个将走分段上传</span>
    `;
  }

  function renderQueue() {
    syncSelectionSummary();
    if (!selectedListEl) return;
    if (!queue.length) {
      selectedListEl.hidden = true;
      selectedListEl.innerHTML = "";
      setUploading(false);
      return;
    }
    selectedListEl.hidden = false;
    selectedListEl.innerHTML = queue
      .map((item) => {
        const token = fileToken(item.file.name, false);
        const percent = Math.max(0, Math.min(100, Math.round(item.progress * 100)));
        const stateLabel =
          item.status === "done"
            ? "完成"
            : item.status === "error"
              ? "失败"
              : item.status === "uploading"
                ? "上传中"
                : "待上传";
        const strategyLabel =
          item.file.size >= CHUNK_THRESHOLD_BYTES ? "分段上传" : "常规上传";
        return `
          <div class="upload-queue-item" data-key="${escapeHtml(item.key)}">
            <div class="upload-queue-item-main">
              <span class="upload-entry-token" data-kind="${escapeHtml(token.kind)}" aria-hidden="true">${escapeHtml(token.label)}</span>
              <div class="upload-queue-copy">
                <div class="upload-queue-title-row">
                  <div class="upload-queue-name" title="${escapeHtml(item.file.name)}">${escapeHtml(item.file.name)}</div>
                  <button
                    type="button"
                    class="upload-queue-remove"
                    data-remove-key="${escapeHtml(item.key)}"
                    ${isUploading ? "disabled" : ""}
                  >
                    移除
                  </button>
                </div>
                <div class="upload-queue-meta">
                  <span>${formatBytes(item.file.size)}</span>
                  <span>${strategyLabel}</span>
                  <span>${stateLabel}</span>
                  ${
                    item.totalChunks
                      ? `<span>${item.uploadedChunks}/${item.totalChunks} 段</span>`
                      : ""
                  }
                </div>
                <div class="upload-queue-progress">
                  <div class="upload-queue-progress-bar" style="width:${percent}%"></div>
                </div>
                <div class="upload-queue-note">${escapeHtml(item.note || "等待开始")}</div>
              </div>
            </div>
          </div>
        `;
      })
      .join("");
    setUploading(isUploading);
  }

  function addFiles(fileList) {
    const nextFiles = Array.from(fileList || []);
    if (!nextFiles.length) return;
    const existing = new Set(queue.map((item) => item.key));
    for (const file of nextFiles) {
      const key = queueKey(file);
      if (existing.has(key)) continue;
      existing.add(key);
      queue.push({
        key,
        file,
        progress: 0,
        uploadedChunks: 0,
        totalChunks: 0,
        status: "idle",
        note: file.size >= CHUNK_THRESHOLD_BYTES
          ? `将自动分段上传（每段 ${formatBytes(CHUNK_SIZE_BYTES)}）`
          : "将使用常规上传",
      });
    }
    renderQueue();
  }

  function readResponseJson(response) {
    return response.json().catch(() => ({}));
  }

  async function uploadDirect(item) {
    item.status = "uploading";
    item.progress = 0.15;
    item.note = "准备常规上传";
    renderQueue();

    const body = new FormData();
    body.append("file", item.file, item.file.name);
    if (selectedDir) {
      body.append("dir", selectedDir);
    }

    const response = await fetch(`/api/upload/${encodeURIComponent(appId)}`, {
      method: "POST",
      body,
    });
    const payload = await readResponseJson(response);
    if (!response.ok) {
      throw new Error(payload.error || `上传失败 (${response.status})`);
    }

    item.progress = 1;
    item.status = "done";
    item.note = "常规上传完成";
    renderQueue();
  }

  async function createChunkSession(item) {
    const response = await fetch(`/api/upload/init/${encodeURIComponent(appId)}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        file_name: item.file.name,
        dir: selectedDir || null,
        size_bytes: item.file.size,
        chunk_size: CHUNK_SIZE_BYTES,
        last_modified_ms: item.file.lastModified || null,
      }),
    });
    const payload = await readResponseJson(response);
    if (!response.ok) {
      throw new Error(payload.error || `初始化分段上传失败 (${response.status})`);
    }
    return payload;
  }

  async function uploadChunked(item) {
    item.status = "uploading";
    item.note = "初始化分段上传";
    renderQueue();

    const initPayload = await createChunkSession(item);
    const uploadId = initPayload.uploadId;
    const chunkSize = Number(initPayload.chunkSize) || CHUNK_SIZE_BYTES;
    const totalChunks = Number(initPayload.totalChunks) || 0;
    const uploadedChunks = new Set(
      Array.isArray(initPayload.uploadedChunks) ? initPayload.uploadedChunks : [],
    );

    item.totalChunks = totalChunks;
    item.uploadedChunks = 0;

    let uploadedBytes = 0;
    for (let index = 0; index < totalChunks; index += 1) {
      const start = index * chunkSize;
      const end = Math.min(start + chunkSize, item.file.size);
      if (uploadedChunks.has(index)) {
        uploadedBytes += end - start;
        item.uploadedChunks += 1;
        item.progress = Math.min(uploadedBytes / item.file.size, 0.98);
        item.note = `续传已跳过第 ${index + 1}/${totalChunks} 段`;
        renderQueue();
        continue;
      }

      item.note = `上传第 ${index + 1}/${totalChunks} 段`;
      renderQueue();
      const chunk = item.file.slice(start, end);
      const response = await fetch(
        `/api/upload/chunk/${encodeURIComponent(appId)}?upload_id=${encodeURIComponent(uploadId)}&index=${index}`,
        {
          method: "PUT",
          headers: {
            "Content-Type": "application/octet-stream",
          },
          body: chunk,
        },
      );
      const payload = await readResponseJson(response);
      if (!response.ok) {
        throw new Error(payload.error || `分段上传失败 (${response.status})`);
      }
      uploadedBytes += end - start;
      item.uploadedChunks += 1;
      item.progress = Math.min(uploadedBytes / item.file.size, 0.98);
      item.note = `已上传 ${item.uploadedChunks}/${totalChunks} 段`;
      renderQueue();
    }

    item.note = "正在合并分段";
    renderQueue();
    const completeResponse = await fetch(
      `/api/upload/complete/${encodeURIComponent(appId)}`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ upload_id: uploadId }),
      },
    );
    const completePayload = await readResponseJson(completeResponse);
    if (!completeResponse.ok) {
      throw new Error(
        completePayload.error || `分段上传完成失败 (${completeResponse.status})`,
      );
    }

    item.progress = 1;
    item.status = "done";
    item.note = "分段上传完成";
    renderQueue();
  }

  async function uploadOne(item) {
    if (item.file.size >= CHUNK_THRESHOLD_BYTES) {
      await uploadChunked(item);
    } else {
      await uploadDirect(item);
    }
  }

  async function uploadAll() {
    if (!queue.length || isUploading) return;
    setUploading(true);
    let successCount = 0;
    let failedCount = 0;
    setStatus(`开始上传 ${queue.length} 个文件`, "info");
    renderQueue();

    for (const item of queue) {
      if (item.status === "done") continue;
      try {
        await uploadOne(item);
        successCount += 1;
      } catch (error) {
        failedCount += 1;
        item.status = "error";
        item.note = error?.message || "上传失败";
        renderQueue();
      }
    }

    if (failedCount === 0) {
      setStatus(`已成功上传 ${successCount} 个文件，正在刷新列表…`, "good");
      window.setTimeout(() => window.location.reload(), 280);
    } else {
      setStatus(
        `上传完成：成功 ${successCount} 个，失败 ${failedCount} 个`,
        "danger",
      );
      setUploading(false);
    }
  }

  form?.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (!queue.length) {
      setStatus("请先选择至少一个文件", "danger");
      return;
    }
    await uploadAll();
  });

  pickBtn?.addEventListener("click", () => {
    if (isUploading) return;
    input?.click();
  });

  input?.addEventListener("change", () => {
    addFiles(input.files);
    input.value = "";
  });

  selectedListEl?.addEventListener("click", (event) => {
    const button = event.target.closest("[data-remove-key]");
    if (!button || isUploading) return;
    const removeKey = button.getAttribute("data-remove-key");
    queue = queue.filter((item) => item.key !== removeKey);
    renderQueue();
    if (!queue.length) {
      setStatus("", "");
    }
  });

  function markDropzone(active) {
    if (!dropzone) return;
    dropzone.classList.toggle("is-dragover", !!active);
  }

  dropzone?.addEventListener("click", () => {
    if (isUploading) return;
    input?.click();
  });
  dropzone?.addEventListener("keydown", (event) => {
    if (isUploading) return;
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      input?.click();
    }
  });
  dropzone?.addEventListener("dragenter", (event) => {
    event.preventDefault();
    markDropzone(true);
  });
  dropzone?.addEventListener("dragover", (event) => {
    event.preventDefault();
    markDropzone(true);
  });
  dropzone?.addEventListener("dragleave", (event) => {
    event.preventDefault();
    const nextTarget = event.relatedTarget;
    if (!dropzone.contains(nextTarget)) {
      markDropzone(false);
    }
  });
  dropzone?.addEventListener("drop", (event) => {
    event.preventDefault();
    markDropzone(false);
    addFiles(event.dataTransfer?.files);
  });

  deleteBtn?.addEventListener("click", async () => {
    if (!selectedFile || !window.confirm(`确认删除 ${selectedFile}？`)) return;
    setStatus("删除中…", "info");
    try {
      const response = await fetch(
        `/api/upload/${encodeURIComponent(appId)}?path=${encodeURIComponent(selectedFile)}`,
        { method: "DELETE" },
      );
      const payload = await readResponseJson(response);
      if (!response.ok) {
        throw new Error(payload.error || `删除失败 (${response.status})`);
      }
      setStatus("已删除，正在刷新…", "good");
      window.location.href = `/apps/upload/${encodeURIComponent(appId)}`;
    } catch (error) {
      setStatus(error?.message || "删除失败", "danger");
    }
  });

  renderQueue();
})();
