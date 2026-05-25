(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (typeof boot.ensureManageSourceBundle === "function") return;

  let loadingPromise = null;

  function perfDisableSet() {
    const raw = [];
    try {
      const query = new URLSearchParams(window.location.search || "");
      raw.push(query.get("mei_perf_disable") || "");
    } catch (_) {}
    const globalValue = window.__MEI_PERF_DISABLE__;
    if (Array.isArray(globalValue)) {
      raw.push(globalValue.join(","));
    } else if (typeof globalValue === "string") {
      raw.push(globalValue);
    }
    return new Set(
      raw
        .join(",")
        .split(",")
        .map((item) => String(item || "").trim().toLowerCase())
        .filter(Boolean)
    );
  }

  function perfDisabled(flag) {
    return perfDisableSet().has(String(flag || "").trim().toLowerCase());
  }

  function normalizeTab(raw) {
    const value = String(raw || "").trim().toLowerCase();
    if (value === "source" || value === "diff" || value === "diagnostics") return value;
    return "preview";
  }

  function tabFromUrl() {
    try {
      const url = new URL(window.location.href);
      return normalizeTab(url.searchParams.get("tab"));
    } catch (_) {
      return "preview";
    }
  }

  function scriptAlreadyLoaded(scriptEl) {
    if (!scriptEl) return false;
    if (scriptEl.readyState === "complete" || scriptEl.readyState === "loaded") {
      return true;
    }
    return scriptEl.complete === true;
  }

  function loadManageSourceBundle() {
    if (boot.manageSourceBundleLoaded === true) {
      return Promise.resolve();
    }
    if (loadingPromise) {
      return loadingPromise;
    }
    loadingPromise = new Promise((resolve, reject) => {
      const existing = document.querySelector('script[data-mei-manage-source-bundle="true"]');
      if (existing) {
        if (scriptAlreadyLoaded(existing)) {
          boot.manageSourceBundleLoaded = true;
          resolve();
          return;
        }
        existing.addEventListener(
          "load",
          () => {
            boot.manageSourceBundleLoaded = true;
            resolve();
          },
          { once: true }
        );
        existing.addEventListener(
          "error",
          () => reject(new Error("manage source bundle load failed")),
          { once: true }
        );
        return;
      }
      const script = document.createElement("script");
      script.src = "/app-bundles/manage-source.js";
      script.async = true;
      script.dataset.meiManageSourceBundle = "true";
      script.onload = () => {
        boot.manageSourceBundleLoaded = true;
        resolve();
      };
      script.onerror = () => {
        reject(new Error("manage source bundle load failed"));
      };
      document.head.appendChild(script);
    }).finally(() => {
      loadingPromise = null;
    });
    return loadingPromise;
  }

  function maybeLoadForTab(tab) {
    const normalized = normalizeTab(tab);
    if (normalized === "source" || normalized === "diff") {
      loadManageSourceBundle().catch((error) => {
        boot.manageSourceBundleError = String(error?.message || error || "unknown error");
      });
    }
  }

  boot.ensureManageSourceBundle = loadManageSourceBundle;

  boot.remountManageSourceAfterSpa = async function remountManageSourceAfterSpa() {
    if (!boot.manageSourceBundleLoaded) {
      return;
    }
    if (typeof boot.mountSourceTreeControls === "function") {
      boot.mountSourceTreeControls();
      return;
    }
    const existing = document.querySelector('script[data-mei-manage-source-bundle="true"]');
    if (existing) {
      existing.remove();
    }
    boot.manageSourceBundleLoaded = false;
    loadingPromise = null;
    await loadManageSourceBundle();
  };

  document.addEventListener("mei:manage-tab-change", (event) => {
    maybeLoadForTab(event?.detail?.tab);
  });

  if (document.readyState === "loading") {
    document.addEventListener(
      "DOMContentLoaded",
      () => {
        if (perfDisabled("manage_source_lazy")) {
          loadManageSourceBundle().catch((error) => {
            boot.manageSourceBundleError = String(error?.message || error || "unknown error");
          });
          return;
        }
        maybeLoadForTab(tabFromUrl());
      },
      { once: true }
    );
  } else {
    if (perfDisabled("manage_source_lazy")) {
      loadManageSourceBundle().catch((error) => {
        boot.manageSourceBundleError = String(error?.message || error || "unknown error");
      });
      return;
    }
    maybeLoadForTab(tabFromUrl());
  }
})();
