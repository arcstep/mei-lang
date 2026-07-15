              ? "失败"
              : item.status === "uploading"
                ? "上传中"
                : "待上传";
        const strategyLabel =
          item.file.size >= CHUNK_THRESHOLD_BYTES ? "分段上传" : "常规上传";
        return `
          <div class="upload-queue-item" data-key="${escapeHtml(item.key)}">
            <div class="upload-queue-item-main">
              <span class="upload-queue-ext" aria-hidden="true">${escapeHtml(token.label)}</span>
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
                ${
                  item.note
                    ? `<div class="upload-queue-note">${escapeHtml(item.note)}</div>`
                    : ""
                }
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
        note:
          file.size >= CHUNK_THRESHOLD_BYTES
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
    const targetDir = currentUploadTargetDir();
    if (targetDir) {
      body.append("dir", targetDir);
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
    item.note = `常规上传完成 → ${payload.path || item.file.name}`;
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
        dir: currentUploadTargetDir() || null,
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
    item.note = `分段上传完成 → ${completePayload.path || item.file.name}`;
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
    setStatus(
      `开始上传 ${queue.length} 个文件到 ${formatRelativeDirLabel(currentUploadTargetDir())}`,
      "info",
    );
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
      const configHref = `/config?app=${encodeURIComponent(appId)}`;
      setStatus(
        `已成功上传 ${successCount} 个文件。可在 Config 将 ops.sources.path 指向 upload/*.xlsx，并在 Runtime 执行 prebuild 使数据链生效。`,
        "good",
      );
      window.setTimeout(() => window.location.reload(), 1200);
    } else {
      setStatus(
        `上传完成：成功 ${successCount} 个，失败 ${failedCount} 个`,
        "danger",
      );
      setUploading(false);
    }
  }

  async function updateSelectedEntryPath() {
    if (!selectedFile || !updatePathBtn || isUploading) return;
    const nextPath = normalizeRelativeDir(updatePathInput?.value || "");
    if (!nextPath) {
      setStatus("请输入新的相对路径", "danger");
      return;
    }
    if (nextPath === normalizeRelativeDir(selectedFile)) {
      setStatus("路径未变化", "info");
      return;
    }
    updatePathBtn.disabled = true;
    setStatus("正在修改路径…", "info");
    try {
      const response = await fetch(`/api/upload/rename/${encodeURIComponent(appId)}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          from_path: selectedFile,
          to_path: nextPath,
        }),
      });
      const payload = await readResponseJson(response);
      if (!response.ok) {
        throw new Error(payload.error || `修改路径失败 (${response.status})`);
      }
      setStatus("路径修改完成，正在跳转…", "good");
      navigateToUploadPath(payload.path || nextPath);
    } catch (error) {
      setStatus(error?.message || "修改路径失败", "danger");
      updatePathBtn.disabled = false;
    }
  }

  function clearStatusWhenIdle() {
    if (!queue.length && !isUploading) {
      setStatus("", "");
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
    clearStatusWhenIdle();
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

  uploadTargetDirInput?.addEventListener("input", syncCurrentDirLabel);
  uploadUseCurrentBtn?.addEventListener("click", () => {
    setDirInputValue(uploadTargetDirInput, selectedDir);
  });

  uploadUseRootBtn?.addEventListener("click", () => {
    setDirInputValue(uploadTargetDirInput, "");
  });
  updatePathBtn?.addEventListener("click", updateSelectedEntryPath);
  updatePathInput?.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      updateSelectedEntryPath();
    }
  });

  root.addEventListener("click", (event) => {
    const button = event.target.closest("[data-fill-dir][data-fill-scope]");
    if (!button) return;
    const dir = button.getAttribute("data-fill-dir") || "";
    setDirInputValue(uploadTargetDirInput, dir);
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
      navigateToUploadPath(parentDirOfPath(selectedFile));
    } catch (error) {
      setStatus(error?.message || "删除失败", "danger");
    }
  });

  syncCurrentDirLabel();
  renderQueue();
})();
