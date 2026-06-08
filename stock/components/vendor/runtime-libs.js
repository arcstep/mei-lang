export const ECHARTS_LOCAL = "/workspace-components/vendor/echarts/echarts.min.js";
export const MAPLIBRE_LOCAL_JS = "/workspace-components/vendor/maplibre/maplibre-gl.js";
export const MAPLIBRE_LOCAL_CSS = "/workspace-components/vendor/maplibre/maplibre-gl.css";
export const MAPLIBRE_LOCAL_GLYPHS =
  "/workspace-components/vendor/maplibre/fonts/{fontstack}/{range}.pbf";

function ensureWindow() {
  if (typeof window === "undefined") {
    throw new Error("runtime libraries require a browser window");
  }
  return window;
}

function ensureDocument() {
  if (typeof document === "undefined") {
    throw new Error("runtime libraries require a document");
  }
  return document;
}

function loadScriptOnce({ src, dataAttr, globalName, errorMessage }) {
  const win = ensureWindow();
  const doc = ensureDocument();
  if (globalName && win[globalName]) {
    return Promise.resolve(win[globalName]);
  }
  const promiseKey = `__meiRuntimeScript:${dataAttr}`;
  if (!win[promiseKey]) {
    win[promiseKey] = new Promise((resolve, reject) => {
      const selector = `script[data-mei-runtime-lib="${dataAttr}"]`;
      const existing = doc.querySelector(selector);
      if (existing) {
        existing.addEventListener("load", () => resolve(globalName ? win[globalName] : undefined), {
          once: true,
        });
        existing.addEventListener(
          "error",
          () => reject(new Error(errorMessage || `${dataAttr} script load failed`)),
          { once: true },
        );
        return;
      }
      const script = doc.createElement("script");
      script.src = src;
      script.async = true;
      script.dataset.meiRuntimeLib = dataAttr;
      script.onload = () => resolve(globalName ? win[globalName] : undefined);
      script.onerror = () => reject(new Error(errorMessage || `${dataAttr} script load failed`));
      doc.head.appendChild(script);
    });
  }
  return win[promiseKey];
}

function loadStylesheetOnce({ href, dataAttr }) {
  const doc = ensureDocument();
  const selector = `link[data-mei-runtime-lib="${dataAttr}"]`;
  if (doc.querySelector(selector)) {
    return;
  }
  const link = doc.createElement("link");
  link.rel = "stylesheet";
  link.href = href;
  link.dataset.meiRuntimeLib = dataAttr;
  doc.head.appendChild(link);
}

export function ensureEChartsGlobal() {
  return loadScriptOnce({
    src: ECHARTS_LOCAL,
    dataAttr: "echarts",
    globalName: "echarts",
    errorMessage: "echarts script load failed",
  });
}

export function ensureMapLibreGlobal() {
  loadStylesheetOnce({
    href: MAPLIBRE_LOCAL_CSS,
    dataAttr: "maplibre-css",
  });
  return loadScriptOnce({
    src: MAPLIBRE_LOCAL_JS,
    dataAttr: "maplibre-js",
    globalName: "maplibregl",
    errorMessage: "maplibre script load failed",
  });
}
