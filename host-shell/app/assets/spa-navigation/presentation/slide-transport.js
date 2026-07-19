/**
 * SlideTransport — Slides Surface native pager (0409).
 * prev / next / goto + page index + keyboard. Not owned by FAB.
 *
 * Important: do NOT MutationObserver-watch compose subtree. Deck materialize /
 * map / metrics mutate constantly; observing them + refreshing caused main-thread
 * freezes ("页面无响应") with empty console.
 */
(() => {
  const boot = (window.__meiLangBoot = window.__meiLangBoot || {});
  const ROOT_ID = "mei-slide-transport";
  const TOC_ID = "mei-slide-transport-toc";

  let refreshQueued = false;
  let refreshing = false;
  let startupTimer = 0;

  function isEditableTarget(target) {
    if (!(target instanceof Element)) return false;
    const tag = String(target.tagName || "").toLowerCase();
    if (tag === "input" || tag === "textarea" || tag === "select") return true;
    if (target.isContentEditable) return true;
    return Boolean(target.closest?.("input, textarea, select, [contenteditable='true']"));
  }

  function deckPages() {
    if (typeof boot.listDeckPages === "function") {
      const nodes = boot.listDeckPages();
      if (Array.isArray(nodes) && nodes.length) return nodes;
    }
    const presentation = window.MeiPresentation;
    if (typeof presentation?.listDeckPages === "function") {
      const nodes = presentation.listDeckPages();
      if (Array.isArray(nodes) && nodes.length) return nodes;
    }
    return Array.from(document.querySelectorAll('[data-mei-ui-role="slide"]')).filter(
      (node) => node instanceof HTMLElement,
    );
  }

  function isPagedSurface() {
    const body = document.body;
    if (body instanceof HTMLElement) {
      const surface = String(body.getAttribute("data-mei-stage-surface") || "");
      const profile = String(body.getAttribute("data-mei-stage-profile") || "");
      if (surface === "paged" || profile === "slides") return true;
    }
    const compose = document.getElementById("mei-compose-root");
    if (compose instanceof HTMLElement) {
      const surface = String(compose.getAttribute("data-mei-stage-surface") || "");
      const profile = String(compose.getAttribute("data-mei-stage-profile") || "");
      if (surface === "paged" || profile === "slides") return true;
    }
    if (typeof boot.stageSurface?.resolveStageMeta === "function") {
      try {
        const meta = boot.stageSurface.resolveStageMeta();
        if (meta?.surface === "paged" || meta?.profile === "slides") return true;
      } catch (_) {}
    }
    // DOM-only fallback (ignore stale presentation_map from a prior stage).
    return deckPages().length >= 2;
  }

  function currentIndex(pages) {
    const list = pages || deckPages();
    const active = list.findIndex((node) => !node.hasAttribute("hidden"));
    if (active >= 0) return active;
    const raw = document.documentElement.getAttribute("data-mei-active-deck-page-index");
    const parsed = Number(raw);
    return Number.isFinite(parsed) ? parsed : 0;
  }

  function slideTitle(node, index) {
    const name = String(node?.getAttribute?.("data-mei-panel-name") || "").trim();
    const leaf = name.split("/").pop() || name;
    return (
      String(node?.getAttribute?.("data-mei-structure-label") || "").trim() ||
      String(node?.getAttribute?.("aria-label") || "").trim() ||
      leaf ||
      `第 ${index + 1} 页`
    );
  }

  function goPrev() {
    if (typeof boot.prevDeckPage === "function") return Boolean(boot.prevDeckPage());
    return Boolean(window.MeiPresentation?.prevPage?.());
  }

  function goNext() {
    if (typeof boot.nextDeckPage === "function") return Boolean(boot.nextDeckPage());
    return Boolean(window.MeiPresentation?.nextPage?.());
  }

  function goTo(index) {
    if (typeof boot.showDeckPage === "function") return Boolean(boot.showDeckPage(index));
    return Boolean(window.MeiPresentation?.showPage?.(index));
  }

  function ensureRoot() {
    let root = document.getElementById(ROOT_ID);
    if (root instanceof HTMLElement) return root;
    root = document.createElement("nav");
    root.id = ROOT_ID;
    root.className = "mei-slide-transport";
    root.setAttribute("aria-label", "幻灯片翻页");
    root.hidden = true;
    root.innerHTML = `
      <button type="button" class="mei-slide-transport__btn" data-action="prev" aria-label="上一页">‹</button>
      <button type="button" class="mei-slide-transport__page" data-action="toc" aria-label="页码与目录" aria-haspopup="listbox" aria-expanded="false">
        <span data-role="label">1 / 1</span>
      </button>
      <button type="button" class="mei-slide-transport__btn" data-action="next" aria-label="下一页">›</button>
      <div id="${TOC_ID}" class="mei-slide-transport__toc" role="listbox" hidden></div>
    `;
    root.addEventListener("click", (event) => {
      const btn = event.target instanceof Element ? event.target.closest("[data-action]") : null;
      if (!(btn instanceof HTMLElement) || !root.contains(btn)) return;
      const action = btn.getAttribute("data-action");
      if (action === "prev") {
        closeToc();
        goPrev();
        scheduleRefresh();
        return;
      }
      if (action === "next") {
        closeToc();
        goNext();
        scheduleRefresh();
        return;
      }
      if (action === "toc") {
        toggleToc();
        return;
      }
      if (action === "goto") {
        const index = Number(btn.getAttribute("data-index"));
        closeToc();
        if (Number.isFinite(index)) goTo(index);
        scheduleRefresh();
      }
    });
    const host =
      document.querySelector(".shell.scene-shell") ||
      document.querySelector("main") ||
      document.body;
    host.appendChild(root);
    return root;
  }

  function tocEl() {
    return document.getElementById(TOC_ID);
  }

  function closeToc() {
    const toc = tocEl();
    const root = document.getElementById(ROOT_ID);
    if (toc) toc.hidden = true;
    const pageBtn = root?.querySelector?.('[data-action="toc"]');
    if (pageBtn instanceof HTMLElement) pageBtn.setAttribute("aria-expanded", "false");
  }

  function toggleToc() {
    const toc = tocEl();
    const root = document.getElementById(ROOT_ID);
    if (!(toc instanceof HTMLElement) || !(root instanceof HTMLElement)) return;
    const open = toc.hidden;
    if (open) {
      const pages = deckPages();
      const active = currentIndex(pages);
      toc.innerHTML = pages
        .map((node, index) => {
          const selected = index === active ? "true" : "false";
          const title = slideTitle(node, index);
          return `<button type="button" role="option" class="mei-slide-transport__toc-item" data-action="goto" data-index="${index}" aria-selected="${selected}">${index + 1}. ${escapeHtml(title)}</button>`;
        })
        .join("");
      toc.hidden = false;
    } else {
      toc.hidden = true;
    }
    const pageBtn = root.querySelector('[data-action="toc"]');
    if (pageBtn instanceof HTMLElement) {
      pageBtn.setAttribute("aria-expanded", open ? "true" : "false");
    }
  }

  function escapeHtml(text) {
    return String(text || "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function syncStageSurfaceOnce() {
    if (typeof boot.stageSurface?.syncFromLocation !== "function") return;
    try {
      boot.stageSurface.syncFromLocation();
    } catch (_) {}
  }

  function refresh(opts = {}) {
    if (refreshing) return false;
    refreshing = true;
    try {
      if (opts.syncSurface) syncStageSurfaceOnce();
      const root = ensureRoot();
      const pages = deckPages();
      const count = pages.length;
      const paged = isPagedSurface();
      if (!paged || count < 1) {
        if (!root.hidden) root.hidden = true;
        closeToc();
        return false;
      }
      if (opts.ensureVisibility && typeof boot.ensureDeckPageVisibility === "function") {
        boot.ensureDeckPageVisibility();
      }
      const index = currentIndex(pages);
      const label = root.querySelector('[data-role="label"]');
      const nextLabel = `${index + 1} / ${count}`;
      if (label && label.textContent !== nextLabel) label.textContent = nextLabel;
      const prev = root.querySelector('[data-action="prev"]');
      const next = root.querySelector('[data-action="next"]');
      if (prev instanceof HTMLButtonElement) prev.disabled = index <= 0;
      if (next instanceof HTMLButtonElement) next.disabled = index >= count - 1;
      const shouldHide = count < 2;
      if (root.hidden !== shouldHide) root.hidden = shouldHide;
      if (root.dataset.count !== String(count)) root.dataset.count = String(count);
      if (root.dataset.index !== String(index)) root.dataset.index = String(index);
      return true;
    } finally {
      refreshing = false;
    }
  }

  function scheduleRefresh(opts = {}) {
    if (refreshQueued) return;
    refreshQueued = true;
    window.requestAnimationFrame(() => {
      refreshQueued = false;
      refresh(opts);
    });
  }

  function onKeyDown(event) {
    if (!isPagedSurface() || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey) {
      return;
    }
    if (isEditableTarget(event.target)) return;
    const pages = deckPages();
    if (pages.length < 2) return;
    const key = event.key;
    if (key === "ArrowLeft" || key === "PageUp" || key === "Backspace") {
      event.preventDefault();
      goPrev();
      scheduleRefresh();
      return;
    }
    if (key === "ArrowRight" || key === "PageDown" || key === " ") {
      event.preventDefault();
      goNext();
      scheduleRefresh();
    }
  }

  function onDocumentClick(event) {
    const root = document.getElementById(ROOT_ID);
    const toc = tocEl();
    if (!(toc instanceof HTMLElement) || toc.hidden) return;
    if (root instanceof HTMLElement && event.target instanceof Node && root.contains(event.target)) {
      return;
    }
    closeToc();
  }

  function install() {
    if (boot.slideTransportMounted) return;
    boot.slideTransportMounted = true;
    ensureRoot();
    document.addEventListener("keydown", onKeyDown, true);
    document.addEventListener("click", onDocumentClick, true);
    document.addEventListener("mei:slide-page-change", () => scheduleRefresh());
    window.addEventListener("popstate", () => {
      scheduleRefresh({ syncSurface: true, ensureVisibility: true });
    });
    // Only watch stage surface attrs on body — never compose subtree.
    if (document.body) {
      const observer = new MutationObserver(() => scheduleRefresh());
      observer.observe(document.body, {
        attributes: true,
        attributeFilter: ["data-mei-stage-surface", "data-mei-stage-profile", "data-mei-stage-id"],
      });
    }
    boot.slideTransport = {
      refresh: () => refresh({ syncSurface: true, ensureVisibility: true }),
      scheduleRefresh,
      prev: goPrev,
      next: goNext,
      goto: goTo,
    };
    refresh({ syncSurface: true, ensureVisibility: true });
    // Compose may arrive after boot; limited soft retries (no tight loop).
    let tries = 0;
    startupTimer = window.setInterval(() => {
      tries += 1;
      refresh({ syncSurface: tries === 1, ensureVisibility: true });
      if (tries >= 12 || (isPagedSurface() && deckPages().length >= 2)) {
        window.clearInterval(startupTimer);
        startupTimer = 0;
      }
    }, 500);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", install, { once: true });
  } else {
    install();
  }
})();
