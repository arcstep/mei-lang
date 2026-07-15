const TRACE_STORE_KEY = "__MEI_RENDER_TRACE__";
const TRACE_ORIGIN_KEY = "__MEI_RENDER_TRACE_ORIGIN_MS__";
const TRACE_LIMIT = 240;
const TRACE_STATE = new WeakMap();

function nowMs() {
  if (typeof performance !== "undefined" && typeof performance.now === "function") {
    return performance.now();
  }
  return Date.now();
}

function ensureWindowStore() {
  if (typeof window === "undefined") {
    return [];
  }
  if (!Array.isArray(window[TRACE_STORE_KEY])) {
    window[TRACE_STORE_KEY] = [];
  }
  if (!Number.isFinite(window[TRACE_ORIGIN_KEY])) {
    window[TRACE_ORIGIN_KEY] = nowMs();
  }
  return window[TRACE_STORE_KEY];
}

function resolveTraceHost() {
  if (typeof document === "undefined") {
    return null;
  }
  try {
    if (window.parent && window.parent !== window) {
      const parentHost = window.parent.document.getElementById("mei-render-trace-diagnostics");
      if (parentHost) {
        return parentHost;
      }
    }
  } catch (_) {
    /* ignore cross-frame access issues */
  }
  return document.getElementById("mei-render-trace-diagnostics");
}

function escapeHtml(value) {
  return String(value ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function renderTraceHost() {
  const host = resolveTraceHost();
  if (!host || typeof window === "undefined") {
    return;
  }
  const entries = Array.isArray(window[TRACE_STORE_KEY]) ? window[TRACE_STORE_KEY] : [];
  if (entries.length === 0) {
    host.textContent = "尚无组件渲染追踪。";
    return;
  }
  host.innerHTML = entries
    .slice(0, 40)
    .map((entry) => {
      const extra = Object.entries(entry.detail || {})
        .filter(([, value]) => value != null && value !== "")
        .slice(0, 6)
        .map(([key, value]) => `${escapeHtml(key)}=${escapeHtml(value)}`)
        .join(" · ");
      return `
        <div style="display:block;margin:4px 0;padding:6px 8px;border-radius:8px;border:1px solid rgba(148,163,184,.25);background:rgba(15,23,42,.34);font-size:11px;line-height:1.45;color:#e2e8f0;">
          <strong style="color:#93c5fd;">${escapeHtml(entry.component)}</strong>
          <span style="color:#f8fafc;"> · ${escapeHtml(entry.phase)}</span>
          <span style="margin-left:6px;color:#94a3b8;">+${escapeHtml(entry.elapsed_ms)}ms</span>
          ${extra ? `<div style="margin-top:4px;color:#94a3b8;">${extra}</div>` : ""}
        </div>
      `;
    })
    .join("");
}

export function clearRenderTrace(reason = "") {
  if (typeof window === "undefined") {
    return;
  }
  window[TRACE_STORE_KEY] = [];
  window[TRACE_ORIGIN_KEY] = nowMs();
  const host = resolveTraceHost();
  if (host) {
    host.textContent = reason ? `尚无组件渲染追踪。（已清空：${reason}）` : "尚无组件渲染追踪。";
  }
}

export function recordRenderTrace(component, phase, detail = {}, state = null) {
  if (typeof window === "undefined") {
    return null;
  }
  const store = ensureWindowStore();
  const currentNow = nowMs();
  const baseState = state || { startMs: currentNow };
  const entry = {
    component: String(component || "unknown"),
    phase: String(phase || "unknown"),
    elapsed_ms: Math.round(currentNow - baseState.startMs),
    since_page_ms: Math.round(currentNow - window[TRACE_ORIGIN_KEY]),
    detail:
      detail && typeof detail === "object" && !Array.isArray(detail)
        ? { ...detail }
        : { value: String(detail ?? "") },
  };
  store.unshift(entry);
  if (store.length > TRACE_LIMIT) {
    store.length = TRACE_LIMIT;
  }
  if (typeof window.__meiLoadingProgressMarkRender === "function") {
    window.__meiLoadingProgressMarkRender(entry);
  }
  renderTraceHost();
  return entry;
}

export function createComponentTracer(host, component, baseDetail = {}) {
  let state = TRACE_STATE.get(host);
  if (!state) {
    state = {
      startMs: nowMs(),
    };
    TRACE_STATE.set(host, state);
  }
  return {
    mark(phase, detail = {}) {
      return recordRenderTrace(
        component,
        phase,
        {
          ...(baseDetail || {}),
          ...(detail && typeof detail === "object" ? detail : {}),
        },
        state,
      );
    },
  };
}

if (typeof window !== "undefined") {
  window.__meiClearRenderTrace = clearRenderTrace;
  ensureWindowStore();
}
