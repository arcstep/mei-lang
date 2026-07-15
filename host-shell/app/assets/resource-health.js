(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.resourceHealthMounted) return;
  boot.resourceHealthMounted = true;

  const BAR_ID = "mei-resource-health-bar";
  const entries = new Map();
  const listeners = new Set();

  function entryKey(entry) {
    return [
      entry.kind || "unknown",
      entry.appId || "",
      entry.id || entry.resourceId || entry.message || "",
      entry.panelId || "",
    ].join("|");
  }

  function ensureBar() {
    let bar = document.getElementById(BAR_ID);
    if (bar) return bar;
    bar = document.createElement("div");
    bar.id = BAR_ID;
    bar.setAttribute("role", "status");
    bar.hidden = true;
    bar.innerHTML =
      '<button type="button" class="mei-rh-summary"></button><div class="mei-rh-detail" hidden></div>';
    document.body.appendChild(bar);
    const summary = bar.querySelector(".mei-rh-summary");
    const detail = bar.querySelector(".mei-rh-detail");
    summary.addEventListener("click", () => {
      detail.hidden = !detail.hidden;
    });
    return bar;
  }

  function render() {
    const bar = ensureBar();
    const list = Array.from(entries.values()).filter(
      (e) => e.severity === "degrade" || e.severity === "blocking",
    );
    const summaryBtn = bar.querySelector(".mei-rh-summary");
    const detail = bar.querySelector(".mei-rh-detail");
    if (!list.length) {
      bar.hidden = true;
      detail.hidden = true;
      detail.innerHTML = "";
      return;
    }
    bar.hidden = false;
    const blocking = list.filter((e) => e.severity === "blocking").length;
    summaryBtn.textContent =
      blocking > 0
        ? `有 ${blocking} 项阻断问题 · 共 ${list.length} 条`
        : `部分资源未就绪（${list.length}）`;
    detail.innerHTML = list
      .map((e) => {
        const hint = e.hint ? `<div class="mei-rh-hint">${escapeHtml(e.hint)}</div>` : "";
        return `<div class="mei-rh-item" data-sev="${escapeHtml(e.severity)}"><strong>${escapeHtml(
          e.kind || "resource",
        )}</strong> ${escapeHtml(e.message || "")}${hint}</div>`;
      })
      .join("");
  }

  function escapeHtml(value) {
    return String(value || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function report(raw) {
    const now = Date.now();
    const entry = {
      id: raw.id || raw.resourceId || entryKey(raw),
      kind: raw.kind || "unknown",
      severity: raw.severity || "degrade",
      message: raw.message || "资源不可用",
      hint: raw.hint || "",
      appId: raw.appId || "",
      panelId: raw.panelId || "",
      recoverable: raw.recoverable !== false,
      recovery: raw.recovery || "",
      source: raw.source || "client",
      firstSeenAt: now,
      lastSeenAt: now,
      count: 1,
    };
    const key = entryKey(entry);
    const prev = entries.get(key);
    if (prev) {
      prev.lastSeenAt = now;
      prev.count += 1;
      prev.message = entry.message;
      prev.hint = entry.hint || prev.hint;
      prev.severity = entry.severity;
    } else {
      entries.set(key, entry);
    }
    render();
    for (const cb of listeners) {
      try {
        cb(snapshot());
      } catch (_) {
        /* ignore */
      }
    }
    return key;
  }

  function clear(idOrPrefix) {
    if (!idOrPrefix) {
      entries.clear();
    } else {
      for (const key of Array.from(entries.keys())) {
        if (key === idOrPrefix || key.startsWith(idOrPrefix) || key.includes(idOrPrefix)) {
          entries.delete(key);
        }
      }
    }
    render();
  }

  function snapshot() {
    return {
      entries: Array.from(entries.values()),
      degradeCount: Array.from(entries.values()).filter((e) => e.severity === "degrade").length,
      blockingCount: Array.from(entries.values()).filter((e) => e.severity === "blocking")
        .length,
    };
  }

  function subscribe(cb) {
    listeners.add(cb);
    return () => listeners.delete(cb);
  }

  function shouldSuppressHttpToast(url, status) {
    const value = String(url || "");
    if (status === 404 && (value.includes("/api/upload/") || value.includes("/workspace-app-assets/"))) {
      return true;
    }
    if (status >= 500 && value.includes("/gis/")) {
      return true;
    }
    if (value.includes("/api/datasets/") && status >= 400) {
      return true;
    }
    return false;
  }

  boot.resourceHealth = {
    report,
    clear,
    snapshot,
    subscribe,
    shouldSuppressHttpToast,
  };

  // Minimal styles (host-shell.css may override later).
  if (!document.getElementById("mei-resource-health-style")) {
    const style = document.createElement("style");
    style.id = "mei-resource-health-style";
    style.textContent = `
#mei-resource-health-bar {
  position: fixed; left: 12px; right: 12px; bottom: 12px; z-index: 9998;
  background: rgba(20, 28, 38, 0.96); color: #e8eef4; border: 1px solid #3d4a5a;
  border-radius: 10px; box-shadow: 0 8px 24px rgba(0,0,0,.35); padding: 0.55rem 0.75rem;
  font: 13px/1.4 system-ui, sans-serif;
}
#mei-resource-health-bar .mei-rh-summary {
  appearance: none; border: 0; background: transparent; color: inherit;
  font: inherit; cursor: pointer; width: 100%; text-align: left; padding: 0;
}
#mei-resource-health-bar .mei-rh-detail { margin-top: 0.55rem; max-height: 40vh; overflow: auto; }
#mei-resource-health-bar .mei-rh-item { padding: 0.35rem 0; border-top: 1px solid #2a3542; }
#mei-resource-health-bar .mei-rh-hint { color: #9aa8b8; font-size: 12px; margin-top: 0.15rem; }
`;
    document.head.appendChild(style);
  }
})();
