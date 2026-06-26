(function initUploadPanel() {
  const root = document.getElementById("upload-panel-root");
  if (!root) return;

  const appId = root.dataset.appId || "";
  const selectedFile = root.dataset.selectedFile || "";
  const selectedDir = root.dataset.selectedDir || "";
  const selectedIsDir = root.dataset.selectedIsDir === "1";
  const uploadRoot = root.dataset.uploadRoot || "upload";
  const fileListEl = document.querySelector(".upload-file-list");
  if (!fileListEl) return;

  const CHUNK_THRESHOLD_BYTES = 16 * 1024 * 1024;
  const CHUNK_SIZE_BYTES = 8 * 1024 * 1024;
  const collator = new Intl.Collator("zh-Hans-CN", {
    numeric: true,
    sensitivity: "base",
  });
  let isUploading = false;
  let queue = [];

  const entryModels = Array.from(fileListEl.querySelectorAll(".upload-file-row"))
    .map((row, index) => {
      const itemEl = row.closest(".upload-file-item") || row.closest("li");
      const path = row.dataset.entryPath || "";
      const name = row.dataset.entryName || path;
      const kind = row.dataset.entryKind || "file";
      return {
        index,
        row,
        itemEl,
        path,
        name,
        isDir: kind === "dir",
        size: Number(row.dataset.entrySize || 0),
        modified: Number(row.dataset.entryModified || 0),
      };
    })
    .filter((entry) => entry.itemEl);

  const directoryOptions = Array.from(
    new Set([
      "",
      ...entryModels.filter((entry) => entry.isDir).map((entry) => entry.path),
    ]),
  );

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

  function normalizeRelativeDir(value) {
    return String(value || "")
      .replaceAll("\\", "/")
      .trim()
      .replace(/^\/+/, "")
      .replace(/\/+/g, "/")
      .replace(/\/+$/, "");
  }

  function formatRelativeDirLabel(dir) {
    const normalized = normalizeRelativeDir(dir);
    return normalized ? `${uploadRoot}/${normalized}` : uploadRoot;
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

  function downloadHrefForPath(relPath) {
    const rel = String(relPath || "").trim();
    if (!rel) return "#";
    return `/api/upload/download/${encodeURIComponent(appId)}?path=${encodeURIComponent(rel)}`;
  }

  function buildDirOptionsHtml() {
    return directoryOptions
      .map((dir) => {
        const label = dir ? formatRelativeDirLabel(dir) : `${uploadRoot}（根目录）`;
        return `<option value="${escapeHtml(dir)}">${escapeHtml(label)}</option>`;
      })
      .join("");
  }

  function buildDirChipsHtml(scope) {
    return directoryOptions
      .map((dir) => {
        const label = dir ? dir : "根目录";
        return `
          <button
            type="button"
            class="upload-folder-chip"
            data-fill-dir="${escapeHtml(dir)}"
            data-fill-scope="${escapeHtml(scope)}"
          >
            ${escapeHtml(label)}
          </button>
        `;
      })
      .join("");
  }

  root.innerHTML = `
    <div class="upload-panel-shell">
      <header class="upload-panel-hero">
        <div class="upload-panel-hero-top">
          <h2 class="upload-panel-hero-title">上传工作台</h2>
          <div id="upload-current-dir-label" class="upload-panel-hero-path">${escapeHtml(formatRelativeDirLabel(selectedDir))}</div>
        </div>
        <p class="upload-panel-hero-note">${
          selectedIsDir
            ? "当前选中目录，新上传的文件将直接落到该路径。"
            : "指定目标路径后，拖拽或选择文件即可开始上传。"
        }</p>
      </header>

      <section class="upload-panel-card">
        <div class="upload-panel-card-head">
          <div class="upload-panel-section-title">上传文件</div>
        </div>
        <label class="upload-panel-field">
          <span class="upload-panel-field-label">目标路径</span>
          <div class="upload-panel-target-inputs">
            <input
              id="upload-target-dir-input"
              class="upload-panel-field-input"
              type="text"
              list="upload-dir-options"
              value="${escapeHtml(selectedDir)}"
              placeholder="留空为根目录，如 media/2026"
            />
            <div class="upload-panel-target-quick">
              <button type="button" id="upload-target-use-current-btn" class="upload-btn upload-btn--secondary">当前选中</button>
              <button type="button" id="upload-target-use-root-btn" class="upload-btn upload-btn--secondary">根目录</button>
            </div>
          </div>
        </label>
        ${
          directoryOptions.length > 1
            ? `
        <div class="upload-panel-dir-quick">
          <span class="upload-panel-field-label">快捷目录</span>
          <div class="upload-folder-chip-list">${buildDirChipsHtml("upload")}</div>
        </div>`
            : ""
        }
        <form id="upload-file-form" class="upload-form">
          <input id="upload-file-input" type="file" name="file" multiple hidden />
          <datalist id="upload-dir-options">${buildDirOptionsHtml()}</datalist>
          <div
            id="upload-dropzone"
            class="upload-dropzone"
            tabindex="0"
            role="button"
            aria-label="拖拽文件到这里或点击选择文件"
          >
            <span class="upload-dropzone-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 16V4m0 0 7 7m-7-7-7 7"></path>
                <path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"></path>
              </svg>
            </span>
            <div class="upload-dropzone-copy">
              <span class="upload-dropzone-label">拖拽文件到此处</span>
              <span class="upload-dropzone-hint">或点击选择 · 超过 ${formatBytes(CHUNK_THRESHOLD_BYTES)} 自动分段上传</span>
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
        selectedFile
          ? `
      <section class="upload-panel-card upload-panel-card--manage">
        <div class="upload-panel-card-head">
          <div class="upload-panel-section-title">管理选中项</div>
          <div class="upload-panel-selected-path" title="${escapeHtml(selectedFile)}">${escapeHtml(selectedFile)}</div>
        </div>
        <label class="upload-panel-field">
          <span class="upload-panel-field-label">新路径</span>
          <input
            id="upload-update-path-input"
            class="upload-panel-field-input"
            type="text"
            value="${escapeHtml(selectedFile)}"
            placeholder="${selectedIsDir ? "例如 archive/2026/new-folder" : "例如 archive/2026/report.csv"}"
          />
        </label>
        <p class="upload-panel-field-note">修改路径可重命名或迁移；中间目录不存在时会自动创建。</p>
        <div class="upload-panel-inline-actions">
          ${
            selectedIsDir
              ? ""
              : `<a
            id="upload-download-btn"
            class="upload-btn upload-btn--secondary"
            href="${escapeHtml(downloadHrefForPath(selectedFile))}"
            download
          >下载</a>`
          }
          <button type="button" id="upload-update-path-btn" class="upload-btn upload-btn--primary">应用路径</button>
          <button type="button" id="upload-delete-btn" class="upload-btn upload-btn--danger">删除</button>
        </div>
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
  const currentDirLabelEl = document.getElementById("upload-current-dir-label");
  const uploadTargetDirInput = document.getElementById("upload-target-dir-input");
  const uploadUseCurrentBtn = document.getElementById("upload-target-use-current-btn");
  const uploadUseRootBtn = document.getElementById("upload-target-use-root-btn");
  const updatePathInput = document.getElementById("upload-update-path-input");
  const updatePathBtn = document.getElementById("upload-update-path-btn");
  const deleteBtn = document.getElementById("upload-delete-btn");

  function currentUploadTargetDir() {
    return normalizeRelativeDir(uploadTargetDirInput?.value || selectedDir);
  }

  function basenameOfPath(path) {
    const normalized = normalizeRelativeDir(path);
    if (!normalized) return "";
    const parts = normalized.split("/").filter(Boolean);
    return parts[parts.length - 1] || "";
  }

  function parentDirOfPath(path) {
    const normalized = normalizeRelativeDir(path);
    if (!normalized) return "";
    const parts = normalized.split("/").filter(Boolean);
    parts.pop();
    return parts.join("/");
  }

  function joinRelativePath(baseDir, childPath) {
    const base = normalizeRelativeDir(baseDir);
    const child = normalizeRelativeDir(childPath);
    if (!base) return child;
    if (!child) return base;
    return `${base}/${child}`;
  }

  function uploadPageHref(path) {
    const normalized = normalizeRelativeDir(path);
    if (!normalized) {
      return `/apps/upload/${encodeURIComponent(appId)}`;
    }
    return `/apps/upload/${encodeURIComponent(appId)}?file=${encodeURIComponent(normalized)}`;
  }

  function navigateToUploadPath(path) {
    window.location.href = uploadPageHref(path);
  }

  function syncCurrentDirLabel() {
    if (currentDirLabelEl) {
      currentDirLabelEl.textContent = formatRelativeDirLabel(currentUploadTargetDir());
    }
  }

  function setDirInputValue(inputEl, value) {
    if (!inputEl) return;
    inputEl.value = normalizeRelativeDir(value);
    if (inputEl === uploadTargetDirInput) {
      syncCurrentDirLabel();
    }
  }

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
    if (updatePathBtn) updatePathBtn.disabled = isUploading;
    if (updatePathInput) updatePathInput.disabled = isUploading;
    if (deleteBtn) deleteBtn.disabled = isUploading;
    if (uploadTargetDirInput) uploadTargetDirInput.disabled = isUploading;
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
