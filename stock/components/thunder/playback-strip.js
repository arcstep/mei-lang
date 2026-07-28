/**
 * Thunder 中栏时间标尺：
 * 左：实时 / 播放 / 速度；右：居中日期(YYYY-MM-DD) · 时间 · 刻度。
 */
import { parseProps, escapeHtml } from "../cockpit/shared.js";
import { COCKPIT_TYPE, cockpitCssVars } from "../cockpit/tokens.js";
import { color } from "../mei/theme-style.js";
import {
  HISTORY_PLAY_RATE_MS,
  LIVE_PLAY_RATE_MS,
  LIVE_WINDOW_MS,
  PLAY_SPEED_OPTIONS,
  getThunderStore,
  liveWindowFromTbiz,
  publishThunderState,
  resolvePlaySpeedOption,
  subscribeThunderState,
} from "./event-bus.js";
import {
  buildWindowTicksHierarchical,
  formatHhMm,
  formatYmd,
  queryThunderDataset,
} from "./pg-query.js";

const POLL_MS = 10_000;
const PLAY_PUBLISH_MS = 80;
/** 标尺左右内边距，避免刻度文字贴控件 */
const RAIL_PAD_PX = 16;

class MeiThunderPlaybackStrip extends HTMLElement {
  connectedCallback() {
    this._props = parseProps(this);
    if (!this.shadowRoot) this.attachShadow({ mode: "open" });
    this.style.display = "block";
    this.style.width = "100%";
    this.style.height = "100%";
    this.style.minHeight = "0";
    this.style.overflow = "hidden";
    this.style.boxSizing = "border-box";
    this._lightningTs = [];
    this._unsub = subscribeThunderState((detail) => this.render(detail));
    this.shadowRoot.addEventListener("click", (e) => this.onClick(e));
    this.shadowRoot.addEventListener("pointerdown", (e) => this.onPointerDown(e));
    this._onDocPointerDown = (e) => {
      if (!this._speedMenuOpen) return;
      const path = typeof e.composedPath === "function" ? e.composedPath() : [];
      if (path.includes(this)) return;
      this._speedMenuOpen = false;
      this.render(getThunderStore());
    };
    document.addEventListener("pointerdown", this._onDocPointerDown, true);
    this.bootstrap();
  }

  disconnectedCallback() {
    this.stopTimers();
    if (typeof this._unsub === "function") {
      this._unsub();
      this._unsub = null;
    }
    if (typeof this._onDocPointerDown === "function") {
      document.removeEventListener("pointerdown", this._onDocPointerDown, true);
      this._onDocPointerDown = null;
    }
  }

  stopTimers() {
    if (this._poll) {
      clearInterval(this._poll);
      this._poll = null;
    }
    this.stopPlayLoop();
  }

  stopPlayLoop() {
    if (this._playRaf != null) {
      cancelAnimationFrame(this._playRaf);
      this._playRaf = null;
    }
    this._playLastWall = 0;
    this._playPhMs = null;
    this._playLastPublish = 0;
  }

  syncPlayTimer(store) {
    const playing = !!store?.playing;
    if (!playing) {
      this.stopPlayLoop();
      return;
    }
    if (this._playRaf != null) return;
    const startPh = new Date(store.playhead || store.windowStart).getTime();
    this._playPhMs = Number.isFinite(startPh) ? startPh : Date.now();
    this._playLastWall = performance.now();
    this._playLastPublish = 0;
    const tick = (now) => {
      this._playRaf = null;
      this.advancePlayheadFrame(now);
      if (getThunderStore().playing) {
        this._playRaf = requestAnimationFrame(tick);
      }
    };
    this._playRaf = requestAnimationFrame(tick);
  }

  advancePlayheadFrame(now) {
    const store = getThunderStore();
    if (!store.playing) {
      this.stopPlayLoop();
      return;
    }

    if (store.mode === "live") {
      if (!this._livePlayRefresh || now - this._livePlayRefresh > 2000) {
        this._livePlayRefresh = now;
        this.refreshTbiz(true).catch(() => {});
      }
      return;
    }

    const end = new Date(store.windowEnd).getTime();
    const start = new Date(store.windowStart).getTime();
    if (!Number.isFinite(end) || !Number.isFinite(start)) return;

    const last = this._playLastWall || now;
    const dtWall = Math.min(100, Math.max(0, now - last));
    this._playLastWall = now;

    const rateMs = Math.max(
      1000,
      Number(store.playSpeed) || HISTORY_PLAY_RATE_MS,
    );
    let ph =
      this._playPhMs != null && Number.isFinite(this._playPhMs)
        ? this._playPhMs
        : new Date(store.playhead || store.windowStart).getTime();
    ph = Math.min(end, ph + (dtWall / 1000) * rateMs);
    this._playPhMs = ph;
    this.patchPlayheadVisual(ph);

    const duePublish =
      !this._playLastPublish || now - this._playLastPublish >= PLAY_PUBLISH_MS;
    const atEnd = ph >= end;
    if (duePublish || atEnd) {
      this._playLastPublish = now;
      publishThunderState(
        {
          playhead: new Date(ph).toISOString(),
          playing: !atEnd,
        },
        "timeline-play",
      );
      if (atEnd) this.stopPlayLoop();
    }
  }

  patchPlayheadVisual(phMs) {
    const store = getThunderStore();
    const w0 = new Date(store.windowStart).getTime();
    const w1 = new Date(store.windowEnd).getTime();
    const span = Math.max(1, w1 - w0);
    const phPct = Math.min(100, Math.max(0, ((phMs - w0) / span) * 100));
    this.shadowRoot?.querySelectorAll("[data-playhead]").forEach((el) => {
      el.style.left = `${phPct}%`;
    });
  }

  async bootstrap() {
    publishThunderState({ selectedSiteIds: [] }, "timeline-sites-all");
    await this.refreshTbiz(true);
    await this.refreshLightningMarks();
    this._poll = setInterval(() => {
      this.refreshTbiz(false).catch(() => {});
      this.refreshLightningMarks().catch(() => {});
    }, POLL_MS);
    this.render(getThunderStore());
  }

  async refreshTbiz(forceLive) {
    try {
      const { rows } = await queryThunderDataset("ts_biz_clock", { pageSize: 1 });
      const raw = rows?.[0]?.t_biz;
      const tBiz = raw ? new Date(raw).toISOString() : new Date().toISOString();
      const store = getThunderStore();
      if (forceLive || store.mode === "live" || !store.windowStart) {
        const live = liveWindowFromTbiz(tBiz, LIVE_WINDOW_MS);
        publishThunderState(
          {
            ...live,
            selectedSiteIds: [],
            playSpeed: LIVE_PLAY_RATE_MS,
            playing: store.mode === "live" ? store.playing : false,
          },
          "timeline-tbiz",
        );
      } else {
        publishThunderState({ tBiz }, "timeline-tbiz-meta");
      }
    } catch (error) {
      console.warn("[thunder.playback-strip] t_biz failed", error);
      if (!getThunderStore().windowStart) {
        publishThunderState(
          liveWindowFromTbiz(new Date().toISOString()),
          "timeline-fallback",
        );
      }
    }
  }

  async refreshLightningMarks() {
    try {
      const { rows } = await queryThunderDataset("ts_lightning_window", {
        pageSize: 8000,
      });
      this._lightningTs = (rows || [])
        .map((r) => new Date(r.ts).getTime())
        .filter((t) => Number.isFinite(t));
      this.render(getThunderStore());
    } catch (error) {
      console.warn("[thunder.playback-strip] lightning marks failed", error);
    }
  }

  onPointerDown(event) {
    const path = event.composedPath();
    const rail = path.find(
      (n) => n instanceof HTMLElement && n.dataset?.timelineRail != null,
    );
    if (!rail) return;
    event.preventDefault();
    const store = getThunderStore();
    const start = new Date(store.windowStart).getTime();
    const end = new Date(store.windowEnd).getTime();
    if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return;

    const pick = (clientX) => {
      const rect = rail.getBoundingClientRect();
      const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
      return new Date(start + ratio * (end - start)).toISOString();
    };

    this._scrubbing = true;
    const iso = pick(event.clientX);
    this._playPhMs = new Date(iso).getTime();
    publishThunderState(
      { playhead: iso, playing: false },
      "timeline-scrub",
    );

    const onMove = (ev) => {
      const next = pick(ev.clientX);
      this._playPhMs = new Date(next).getTime();
      publishThunderState({ playhead: next }, "timeline-scrub");
    };
    const onUp = () => {
      this._scrubbing = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  onClick(event) {
    const path = event.composedPath();
    const speedOpt = path.find(
      (node) => node instanceof HTMLElement && node.dataset?.speedMs != null,
    );
    if (speedOpt) {
      const rateMs = Math.max(1000, Number(speedOpt.dataset.speedMs) || HISTORY_PLAY_RATE_MS);
      publishThunderState({ playSpeed: rateMs }, "timeline-speed");
      this._speedMenuOpen = false;
      this.render(getThunderStore());
      return;
    }

    const btn = path.find(
      (node) => node instanceof HTMLElement && node.dataset?.action,
    );
    if (!btn) {
      if (this._speedMenuOpen) {
        this._speedMenuOpen = false;
        this.render(getThunderStore());
      }
      return;
    }
    const action = btn.dataset.action;
    const store = getThunderStore();

    if (action === "live") {
      this._speedMenuOpen = false;
      publishThunderState(
        {
          playing: false,
          selectedSiteIds: [],
          playSpeed: LIVE_PLAY_RATE_MS,
        },
        "timeline-live-prep",
      );
      this.refreshTbiz(true).catch(() => {});
      return;
    }

    if (action === "play") {
      this._speedMenuOpen = false;
      if (store.mode === "live") {
        publishThunderState({ playing: !store.playing }, "timeline-play-toggle");
      } else {
        let ph = new Date(store.playhead || store.windowStart).getTime();
        const end = new Date(store.windowEnd).getTime();
        const start = new Date(store.windowStart).getTime();
        if (Number.isFinite(ph) && Number.isFinite(end) && ph >= end) {
          ph = start;
        }
        publishThunderState(
          {
            playing: !store.playing,
            playhead: Number.isFinite(ph)
              ? new Date(ph).toISOString()
              : store.playhead,
            playSpeed: Number(store.playSpeed) || HISTORY_PLAY_RATE_MS,
          },
          "timeline-play-toggle",
        );
      }
      this.syncPlayTimer(getThunderStore());
      return;
    }

    if (action === "speed") {
      this._speedMenuOpen = !this._speedMenuOpen;
      this.render(getThunderStore());
    }
  }

  updateAxisOnly(store) {
    const w0 = new Date(store.windowStart).getTime();
    const w1 = new Date(store.windowEnd).getTime();
    const span = Math.max(1, w1 - w0);
    const livePh =
      store.playing && this._playPhMs != null && Number.isFinite(this._playPhMs)
        ? this._playPhMs
        : new Date(store.playhead || store.windowEnd).getTime();
    const phPct = Number.isFinite(livePh)
      ? Math.min(100, Math.max(0, ((livePh - w0) / span) * 100))
      : 100;
    this.shadowRoot.querySelectorAll("[data-playhead]").forEach((el) => {
      el.style.left = `${phPct}%`;
    });

    const marksHost = this.shadowRoot.querySelector("[data-marks]");
    if (marksHost && !store.playing) {
      const ph = livePh;
      marksHost.innerHTML = (this._lightningTs || [])
        .filter((t) => t >= w0 && t <= w1)
        .map((t) => {
          const pct = ((t - w0) / span) * 100;
          const faded = Number.isFinite(ph) && t < ph - LIVE_WINDOW_MS;
          if (Number.isFinite(ph) && t > ph) return "";
          return `<span class="mark${faded ? " is-faded" : ""}" style="left:${pct}%"></span>`;
        })
        .join("");
    }

    const playBtn = this.shadowRoot.querySelector('[data-action="play"]');
    if (playBtn) {
      playBtn.textContent = store.playing ? "暂停" : "播放";
      playBtn.classList.toggle("is-on", !!store.playing);
    }
    const liveBtn = this.shadowRoot.querySelector('[data-action="live"]');
    if (liveBtn) {
      liveBtn.classList.toggle("is-on", store.mode === "live");
      liveBtn.classList.toggle("is-dim", store.mode !== "live");
    }
    const speedBtn = this.shadowRoot.querySelector('[data-action="speed"]');
    if (speedBtn) {
      const opt = resolvePlaySpeedOption(store.playSpeed);
      speedBtn.textContent = opt.label;
      speedBtn.title = `播放速度 ${opt.label}`;
      speedBtn.classList.toggle("is-on", !!this._speedMenuOpen);
    }
    const dateEl = this.shadowRoot.querySelector("[data-date-center]");
    if (dateEl) {
      const dateIso =
        store.playing && this._playPhMs != null && Number.isFinite(this._playPhMs)
          ? new Date(this._playPhMs)
          : store.playhead || store.windowEnd || store.windowStart;
      dateEl.textContent = formatYmd(dateIso);
    }
  }

  render(state) {
    const store = state || getThunderStore();
    this.syncPlayTimer(store);

    const hasRail = !!this.shadowRoot?.querySelector("[data-timeline-rail]");
    if (hasRail && (this._scrubbing || store.playing)) {
      this.updateAxisOnly(store);
      return;
    }

    const ticks = buildWindowTicksHierarchical(
      store.windowStart,
      store.windowEnd,
    );
    const w0 = new Date(store.windowStart).getTime();
    const w1 = new Date(store.windowEnd).getTime();
    const span = Math.max(1, w1 - w0);
    const ph = new Date(store.playhead || store.windowEnd).getTime();
    const phPct = Number.isFinite(ph)
      ? Math.min(100, Math.max(0, ((ph - w0) / span) * 100))
      : 100;
    const centerDate = formatYmd(
      Number.isFinite(ph) ? ph : store.windowEnd || store.windowStart,
    );
    const speedOpt = resolvePlaySpeedOption(store.playSpeed);
    const speedMenuOpen = !!this._speedMenuOpen;

    const marks = (this._lightningTs || [])
      .filter((t) => t >= w0 && t <= w1)
      .map((t) => {
        const pct = ((t - w0) / span) * 100;
        const faded = Number.isFinite(ph) && t < ph - LIVE_WINDOW_MS;
        if (Number.isFinite(ph) && t > ph) return "";
        return `<span class="mark${faded ? " is-faded" : ""}" style="left:${pct}%"></span>`;
      })
      .join("");

    const timeRow = ticks
      .filter((t) => t.label)
      .map((tick) => {
        const pct = ((tick.at.getTime() - w0) / span) * 100;
        return `<span class="time-lab" style="left:${pct}%">${escapeHtml(
          formatHhMm(tick.at),
        )}</span>`;
      })
      .join("");

    const tickRow = ticks
      .map((tick) => {
        const pct = ((tick.at.getTime() - w0) / span) * 100;
        const cls = tick.major ? "tick is-major" : "tick is-minor";
        return `<span class="${cls}" style="left:${pct}%"><i></i></span>`;
      })
      .join("");

    const speedMenu = PLAY_SPEED_OPTIONS.map((opt) => {
      const on = opt.rateMs === speedOpt.rateMs;
      return `<button type="button" class="speed-opt${on ? " is-on" : ""}" data-speed-ms="${opt.rateMs}">${escapeHtml(opt.label)}</button>`;
    }).join("");

    const liveOn = store.mode === "live";

    this.shadowRoot.innerHTML = `
      <style>
        :host {
          display: block;
          width: 100%;
          height: 100%;
          min-height: 0;
          overflow: visible;
          box-sizing: border-box;
          margin: 0;
          padding: 0;
          border: none;
          border-radius: 0 !important;
          background: transparent;
          ${cockpitCssVars()}
          font-family: var(--cockpit-font-family-ui);
          font-size: ${COCKPIT_TYPE.chartLabel};
          color: ${color("text_value")};
        }
        *, *::before, *::after { border-radius: 0 !important; }
        .wrap {
          display: grid;
          grid-template-columns: auto minmax(0, 1fr);
          gap: 10px;
          align-items: stretch;
          width: 100%;
          height: 100%;
          min-height: 0;
          margin: 0;
          padding: 0;
          box-sizing: border-box;
        }
        .controls {
          position: relative;
          display: flex;
          flex-direction: column;
          gap: 4px;
          justify-content: center;
          min-width: 64px;
        }
        .btn {
          display: block;
          width: 100%;
          box-sizing: border-box;
          border: 1px solid rgba(56, 160, 240, 0.45);
          background: rgba(8, 24, 48, 0.55);
          color: ${color("text_muted")};
          font-size: ${COCKPIT_TYPE.chartLabel};
          font-family: inherit;
          line-height: 1.15;
          padding: 4px 8px;
          cursor: pointer;
          border-radius: 0 !important;
          white-space: nowrap;
        }
        .btn.is-on {
          color: ${color("text_value")};
          border-color: rgba(56, 189, 248, 0.8);
          background: rgba(14, 60, 110, 0.7);
        }
        .btn.is-dim {
          opacity: 0.45;
          color: ${color("text_muted")};
          border-color: rgba(56, 160, 240, 0.28);
        }
        .speed-wrap {
          position: relative;
        }
        .speed-menu {
          position: absolute;
          left: calc(100% + 6px);
          bottom: 0;
          z-index: 20;
          display: ${speedMenuOpen ? "flex" : "none"};
          flex-direction: column;
          gap: 2px;
          min-width: 56px;
          padding: 4px;
          background: rgba(8, 24, 48, 0.94);
          border: 1px solid rgba(56, 189, 248, 0.55);
          box-shadow: 0 6px 18px rgba(0, 0, 0, 0.35);
        }
        .speed-opt {
          display: block;
          width: 100%;
          box-sizing: border-box;
          border: 1px solid transparent;
          background: transparent;
          color: ${color("text_muted")};
          font-size: ${COCKPIT_TYPE.chartLabel};
          font-family: inherit;
          line-height: 1.2;
          padding: 4px 8px;
          cursor: pointer;
          text-align: center;
          font-variant-numeric: tabular-nums;
        }
        .speed-opt:hover {
          color: ${color("text_value")};
          background: rgba(14, 60, 110, 0.55);
        }
        .speed-opt.is-on {
          color: ${color("text_value")};
          border-color: rgba(56, 189, 248, 0.65);
          background: rgba(14, 60, 110, 0.7);
        }
        .axis {
          min-width: 0;
          min-height: 0;
          height: 100%;
          display: flex;
          flex-direction: column;
          margin: 0;
          padding: 0 ${RAIL_PAD_PX}px;
          box-sizing: border-box;
        }
        .rail {
          position: relative;
          flex: 1 1 auto;
          min-height: 0;
          display: grid;
          /* 日期 / 刻度文字 / 刻度线 等高 1:1:1，避免刻度区吞掉剩余高度 */
          grid-template-rows: 1fr 1fr 1fr;
          gap: 0;
          cursor: ew-resize;
          touch-action: none;
        }
        .lane {
          position: relative;
          min-height: 0;
          overflow: visible;
        }
        .lane-ticks {
          border-bottom: 1px solid rgba(148, 163, 184, 0.65);
        }
        .date-lab, .time-lab {
          position: absolute;
          top: 50%;
          transform: translate(-50%, -50%);
          white-space: nowrap;
          pointer-events: none;
          font-variant-numeric: tabular-nums;
          text-shadow: 0 1px 2px rgba(0, 0, 0, 0.7);
        }
        .date-lab {
          left: 50%;
          font-size: ${COCKPIT_TYPE.chartLabel};
          font-weight: 600;
          color: rgba(186, 230, 253, 0.95);
        }
        .time-lab {
          font-size: ${COCKPIT_TYPE.chartLabel};
          font-weight: 500;
          color: rgba(226, 232, 240, 0.9);
        }
        .tick {
          position: absolute;
          bottom: 0;
          transform: translateX(-50%);
          pointer-events: none;
        }
        .tick i {
          display: block;
          width: 1px;
          margin: 0;
        }
        .tick.is-minor i {
          height: 3px;
          background: rgba(148, 163, 184, 0.4);
        }
        .tick.is-major i {
          height: 6px;
          background: rgba(186, 230, 253, 0.72);
        }
        .mark {
          position: absolute;
          bottom: 7px;
          width: 2px;
          height: 4px;
          margin-left: -1px;
          background: rgba(248, 113, 113, 0.85);
          pointer-events: none;
          z-index: 1;
        }
        .mark.is-faded { opacity: 0.28; background: #94a3b8; }
        .playhead {
          position: absolute;
          top: 0;
          bottom: 0;
          width: 1px;
          background: #38bdf8;
          pointer-events: none;
          z-index: 3;
        }
        .playhead::after {
          content: "";
          position: absolute;
          top: 0;
          left: -4px;
          border-left: 4px solid transparent;
          border-right: 4px solid transparent;
          border-top: 5px solid #38bdf8;
        }
      </style>
      <div class="wrap">
        <div class="controls">
          <button type="button" class="btn${liveOn ? " is-on" : " is-dim"}" data-action="live" title="回到实时">实时</button>
          <button type="button" class="btn${store.playing ? " is-on" : ""}" data-action="play">${
            store.playing ? "暂停" : "播放"
          }</button>
          <div class="speed-wrap">
            <button type="button" class="btn${speedMenuOpen ? " is-on" : ""}" data-action="speed" title="播放速度 ${escapeHtml(speedOpt.label)}">${escapeHtml(speedOpt.label)}</button>
            <div class="speed-menu" role="menu" aria-label="播放速度">${speedMenu}</div>
          </div>
        </div>
        <div class="axis">
          <div class="rail" data-timeline-rail title="拖动游标">
            <div class="lane lane-date"><span class="date-lab" data-date-center>${escapeHtml(centerDate)}</span></div>
            <div class="lane lane-time">${timeRow}</div>
            <div class="lane lane-ticks">
              ${tickRow}
              <span data-marks>${marks}</span>
              <span class="playhead" data-playhead style="left:${phPct}%"></span>
            </div>
          </div>
        </div>
      </div>
    `;
  }
}

if (!customElements.get("mei-thunder-playback-strip")) {
  customElements.define("mei-thunder-playback-strip", MeiThunderPlaybackStrip);
}
