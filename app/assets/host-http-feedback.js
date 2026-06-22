(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  if (boot.hostHttpFeedbackMounted) return;
  boot.hostHttpFeedbackMounted = true;

  const ROOT_ID = "mei-host-http-feedback-root";
  const DEDUPE_MS = 4000;
  const recent = new Map();

  function ensureRoot() {
    let root = document.getElementById(ROOT_ID);
    if (!root) {
      root = document.createElement("div");
      root.id = ROOT_ID;
      root.setAttribute("aria-live", "polite");
      root.setAttribute("aria-relevant", "additions");
      document.body.appendChild(root);
    }
    return root;
  }

  function statusTitle(status) {
    if (status === 401) return "需要登录";
    if (status === 403) return "访问被拒绝";
    if (status === 404) return "资源不存在";
    if (status >= 500) return "服务器错误";
    if (status >= 400) return "请求失败";
    return "HTTP 异常";
  }

  function requestUrlFromInput(input) {
    if (typeof input === "string") return input;
    if (input && typeof input.url === "string") return input.url;
    return "";
  }

  function isSameOriginApiRequest(url) {
    if (!url) return false;
    if (url.startsWith("/")) return url.includes("/api/");
    try {
      const parsed = new URL(url, window.location.origin);
      return (
        parsed.origin === window.location.origin && parsed.pathname.includes("/api/")
      );
    } catch (_) {
      return false;
    }
  }

  function isAuthFlowRequest(url) {
    const value = String(url || "");
    return (
      value.includes("/api/auth/login") ||
      value.includes("/api/auth/public-key") ||
      value.includes("/api/auth/session")
    );
  }

  function redirectToLogin(reason) {
    const auth = window.MeiHostAuthSession;
    if (auth && typeof auth.redirectToLogin === "function") {
      return auth.redirectToLogin(reason);
    }
    const path = window.location.pathname || "";
    if (path === "/login" || path.startsWith("/login/")) return false;
    const next = path + window.location.search + window.location.hash;
    window.location.assign(
      "/login?next=" + encodeURIComponent(next && next !== "/login" ? next : "/"),
    );
    return true;
  }

  async function recoverUnauthorizedRequest(input, init, nativeFetch) {
    const requestUrl = requestUrlFromInput(input);
    if (!isSameOriginApiRequest(requestUrl)) return null;
    if (isAuthFlowRequest(requestUrl) || requestUrl.includes("/api/auth/refresh")) {
      return null;
    }
    const auth = window.MeiHostAuthSession;
    if (!auth || typeof auth.recoverSessionForRequest !== "function") {
      redirectToLogin("session_expired");
      return null;
    }
    const recovered = await auth.recoverSessionForRequest();
    if (!recovered) return null;
    return nativeFetch(input, init);
  }

  function shouldSkipNotify(url, status) {
    if (Number(status) === 401) return true;
    if (!url || !String(url).includes("/api/")) return true;
    if (String(url).includes("/api/host/heartbeat")) return true;
    if (String(url).includes("/api/auth/refresh")) return true;
    if (
      Number(status) === 410 &&
      /\/api\/agent\/session\/[^/?]+\/(diff|revert|unrevert)(?:\?|$)/.test(String(url))
    ) {
      return true;
    }
    const key = String(status) + " " + String(url);
    const now = Date.now();
    const last = recent.get(key) || 0;
    if (now - last < DEDUPE_MS) return true;
    recent.set(key, now);
    return false;
  }

  function parseErrorDetail(text) {
    const raw = String(text || "").trim();
    if (!raw) return "";
    try {
      const payload = JSON.parse(raw);
      if (payload && typeof payload === "object") {
        return String(payload.error || payload.message || "").trim();
      }
    } catch (_) {}
    return raw.length > 240 ? raw.slice(0, 240) + "…" : raw;
  }

  function notify(payload) {
    const status = Number(payload && payload.status) || 0;
    if (!status) return;
    const url = String((payload && payload.url) || "");
    if (shouldSkipNotify(url, status)) return;

    const title = String((payload && payload.title) || statusTitle(status));
    const message = String((payload && payload.message) || "").trim() || "宿主拒绝了本次请求。";
    const tone = status >= 500 || status === 403 || status === 401 ? "error" : "warn";
    const root = ensureRoot();
    const banner = document.createElement("div");
    banner.className =
      "mei-host-http-banner" + (tone === "warn" ? " mei-host-http-banner--warn" : "");
    banner.innerHTML =
      '<div class="mei-host-http-banner__code">HTTP ' +
      String(status) +
      '</div><div class="mei-host-http-banner__body"><div class="mei-host-http-banner__title"></div><div class="mei-host-http-banner__message"></div><div class="mei-host-http-banner__hint"></div></div><button type="button" class="mei-host-http-banner__close" aria-label="关闭">×</button>';
    banner.querySelector(".mei-host-http-banner__title").textContent = title;
    banner.querySelector(".mei-host-http-banner__message").textContent = message;
    banner.querySelector(".mei-host-http-banner__hint").textContent =
      "请向管理员反馈错误代码 HTTP " + String(status) + "。";
    banner.querySelector(".mei-host-http-banner__close").addEventListener("click", () => {
      banner.remove();
    });
    root.prepend(banner);
    window.setTimeout(() => {
      if (banner.isConnected) banner.remove();
    }, 12000);
  }

  function isBuildViewPage() {
    try {
      return window.location.pathname.startsWith("/apps/build/");
    } catch (_) {
      return false;
    }
  }

  function mergeBuildViewFetch(input, init) {
    if (!isBuildViewPage()) return { input, init };
    let requestUrl = "";
    if (typeof input === "string") {
      requestUrl = input;
    } else if (input && typeof input.url === "string") {
      requestUrl = input.url;
    }
    if (!requestUrl.includes("/api/")) return { input, init };
    if (typeof Request !== "undefined" && input instanceof Request) {
      const headers = new Headers(input.headers);
      if (!headers.has("X-Mei-Build-View")) {
        headers.set("X-Mei-Build-View", "1");
      }
      return { input: new Request(input, { headers }), init: undefined };
    }
    const nextInit = { ...(init || {}) };
    const headers = new Headers(nextInit.headers);
    if (!headers.has("X-Mei-Build-View")) {
      headers.set("X-Mei-Build-View", "1");
    }
    nextInit.headers = headers;
    return { input, init: nextInit };
  }

  const nativeFetch = window.fetch.bind(window);
  window.fetch = async function meiHostFetch(input, init) {
    const merged = mergeBuildViewFetch(input, init);
    let response = await nativeFetch(merged.input, merged.init);
    const requestUrl = requestUrlFromInput(merged.input);
    if (
      response.status === 401 &&
      isSameOriginApiRequest(requestUrl) &&
      !isAuthFlowRequest(requestUrl)
    ) {
      const retried = await recoverUnauthorizedRequest(
        merged.input,
        merged.init,
        nativeFetch,
      );
      if (retried) {
        response = retried;
      } else {
        redirectToLogin("session_expired");
      }
    }
    try {
      if (
        requestUrl &&
        (requestUrl.startsWith("/") || requestUrl.startsWith(window.location.origin)) &&
        requestUrl.includes("/api/")
      ) {
        if (!response.ok) {
          let detail = "";
          try {
            detail = parseErrorDetail(await response.clone().text());
          } catch (_) {}
          notify({
            status: response.status,
            url: requestUrl,
            message: detail || response.statusText || "请求失败",
          });
        }
      }
    } catch (_) {}
    return response;
  };

  window.MeiHostHttpFeedback = { notify, statusTitle };
  boot.notifyHostHttpError = notify;
})();
