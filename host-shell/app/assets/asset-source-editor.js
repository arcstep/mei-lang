(() => {
  let editor = null;

  function extOf(target) {
    const s = String(target || "").trim();
    const i = s.lastIndexOf(".");
    return i < 0 ? "" : s.slice(i + 1).toLowerCase();
  }

  function modeFromDataset(lang, target) {
    const l = String(lang || "plain").trim().toLowerCase();
    const ext = extOf(target);
    if (l === "mei" || ext === "mei" || ext === "star") return "mei";
    if (l === "json" || ext === "json" || ext === "jsonc") {
      return { name: "javascript", json: true };
    }
    if (l === "typescript" || ext === "ts" || ext === "tsx") {
      return { name: "javascript", typescript: true };
    }
    if (l === "javascript" || ext === "js" || ext === "jsx" || ext === "mjs" || ext === "cjs") {
      return "javascript";
    }
    if (l === "css" || ext === "css" || ext === "scss" || ext === "less") return "css";
    if (l === "python" || ext === "py" || ext === "pyi") return "python";
    if (l === "xml" || ext === "xml" || ext === "svg") {
      return { name: "xml", htmlMode: false };
    }
    if (l === "html" || ext === "html" || ext === "htm") {
      return { name: "xml", htmlMode: true };
    }
    return null;
  }

  function destroy() {
    if (!editor) return;
    try {
      const wrapper = editor.getWrapperElement();
      if (wrapper && wrapper.parentNode) wrapper.parentNode.removeChild(wrapper);
    } catch (_) {}
    editor = null;
  }

  function boot() {
    const host = document.getElementById("asset-source-editor-host");
    const raw = document.getElementById("asset-source-raw");
    if (!host || !raw || !window.CodeMirror) return;
    const target = String(raw.dataset.sourceTarget || "").trim();
    const lang = String(raw.dataset.sourceLang || "plain").trim();
    const text = String(raw.textContent || "");
    destroy();
    host.innerHTML = "";
    editor = window.CodeMirror(host, {
      value: text,
      lineNumbers: true,
      readOnly: true,
      mode: modeFromDataset(lang, target),
      theme: "default",
      lineWrapping: false,
      scrollbarStyle: "native",
    });
    window.setTimeout(function () {
      if (editor && typeof editor.refresh === "function") editor.refresh();
    }, 30);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot, { once: true });
  } else {
    boot();
  }
  document.addEventListener("mei:manage-tab-change", boot);
  document.addEventListener("mei:manage-context-change", boot);
})();
