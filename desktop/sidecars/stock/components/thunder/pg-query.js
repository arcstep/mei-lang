/**
 * Thunder cockpit → Host datasets/query 轻量客户端（不依赖 metric __mei_runtime_ref）。
 * 需 MEI_APP_THUNDER_DSN；失败时抛错，由调用方展示明确失败态。
 */

const APP_ID = "thunder";

export function levelToZh(level) {
  const raw = String(level || "").trim().toLowerCase();
  if (raw === "red" || raw.includes("红")) return "红";
  if (raw === "orange" || raw.includes("橙")) return "橙";
  if (raw === "yellow" || raw.includes("黄")) return "黄";
  if (raw === "none") return "无";
  return String(level || "").trim() || "—";
}

export function levelFromZh(level) {
  const text = String(level || "").trim();
  if (text.includes("红") || text.toLowerCase() === "red") return "red";
  if (text.includes("橙") || text.toLowerCase() === "orange") return "orange";
  if (text.includes("黄") || text.toLowerCase() === "yellow") return "yellow";
  return "none";
}

/** 10 分钟网格 floor（与 docs/32 floor_10min 一致） */
export function floor10min(isoOrDate) {
  const d = isoOrDate instanceof Date ? isoOrDate : new Date(isoOrDate);
  if (Number.isNaN(d.getTime())) return null;
  const ms = 600_000;
  return new Date(Math.floor(d.getTime() / ms) * ms);
}

export function formatHhMm(isoOrDate) {
  const d = isoOrDate instanceof Date ? isoOrDate : new Date(isoOrDate);
  if (Number.isNaN(d.getTime())) return "";
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  return `${hh}:${mm}`;
}

/** 日期栏：7月28日；跨年则带年份 */
export function formatDateLane(isoOrDate, refYear) {
  const d = isoOrDate instanceof Date ? isoOrDate : new Date(isoOrDate);
  if (Number.isNaN(d.getTime())) return "";
  const y = d.getFullYear();
  const mo = d.getMonth() + 1;
  const da = d.getDate();
  if (refYear != null && y !== Number(refYear)) {
    return `${y}年${mo}月${da}日`;
  }
  return `${mo}月${da}日`;
}

/** 标尺日期栏：YYYY-MM-DD */
export function formatYmd(isoOrDate) {
  const d = isoOrDate instanceof Date ? isoOrDate : new Date(isoOrDate);
  if (Number.isNaN(d.getTime())) return "";
  const y = d.getFullYear();
  const mo = String(d.getMonth() + 1).padStart(2, "0");
  const da = String(d.getDate()).padStart(2, "0");
  return `${y}-${mo}-${da}`;
}

/**
 * 按窗长自动选次刻度（分钟）：无需手工选刻度。
 * ≤45min→1；≤3h→5；≤12h→10；否则 30
 */
export function autoZoomMinutes(windowStart, windowEnd) {
  const a = new Date(windowStart).getTime();
  const b = new Date(windowEnd).getTime();
  if (!Number.isFinite(a) || !Number.isFinite(b) || b <= a) return 1;
  const spanMin = (b - a) / 60_000;
  if (spanMin <= 45) return 1;
  if (spanMin <= 180) return 5;
  if (spanMin <= 720) return 10;
  return 30;
}

/** 快进默认步长：与自动主刻度一致 */
export function autoMajorStepMs(windowStart, windowEnd) {
  const zoom = autoZoomMinutes(windowStart, windowEnd);
  const majorMin = zoom <= 1 ? 5 : zoom <= 5 ? 15 : zoom <= 10 ? 30 : 60;
  return majorMin * 60_000;
}

export function formatWarnedAt(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return String(iso || "").trim();
  const y = d.getFullYear();
  const mo = String(d.getMonth() + 1).padStart(2, "0");
  const da = String(d.getDate()).padStart(2, "0");
  return `${y}-${mo}-${da} ${formatHhMm(d)}`;
}

/**
 * 相对业务时钟（T_biz / 窗右端）的语义时间，不用系统时钟。
 * @param {string|Date} iso
 * @param {string|Date} [nowIso] 缺省则无法计算时回退 formatWarnedAt
 */
export function formatRelativeAgo(iso, nowIso) {
  const t = iso instanceof Date ? iso : new Date(iso);
  const now = nowIso instanceof Date ? nowIso : new Date(nowIso || NaN);
  if (Number.isNaN(t.getTime())) return String(iso || "").trim();
  if (Number.isNaN(now.getTime())) return formatWarnedAt(iso);

  const diffMs = now.getTime() - t.getTime();
  if (diffMs < -60_000) {
    const ahead = Math.abs(diffMs);
    if (ahead < 3600_000) return `${Math.max(1, Math.round(ahead / 60_000))} 分钟后`;
    if (ahead < 86400_000) return `${Math.max(1, Math.round(ahead / 3600_000))} 小时后`;
    return `${Math.max(1, Math.round(ahead / 86400_000))} 天后`;
  }
  if (diffMs < 60_000) return "现在";

  const mins = Math.floor(diffMs / 60_000);
  if (mins < 60) return `${mins} 分钟前`;
  const hours = Math.floor(diffMs / 3600_000);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(diffMs / 86400_000);
  if (days < 30) return `${days} 天前`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months} 个月前`;
  const years = Math.floor(days / 365);
  return `${Math.max(1, years)} 年前`;
}

/** T_biz 近窗、按 zoomMinutes 刻度（含端点） */
export function buildRealtimeTicks(tBiz, hours = 0.5, zoomMinutes = 5) {
  const step = Math.max(1, Number(zoomMinutes) || 5) * 60_000;
  const end = new Date(tBiz || Date.now());
  if (Number.isNaN(end.getTime())) return [];
  const start = new Date(end.getTime() - hours * 3600_000);
  const ticks = [];
  const t0 = Math.floor(start.getTime() / step) * step;
  for (let t = t0; t <= end.getTime() + 1; t += step) {
    if (t >= start.getTime() - 1) ticks.push(new Date(t));
  }
  return ticks;
}

/** 任意窗内刻度（平面列表，兼容旧调用） */
export function buildWindowTicks(windowStart, windowEnd, zoomMinutes = 5) {
  return buildWindowTicksHierarchical(windowStart, windowEnd, zoomMinutes).map(
    (t) => t.at,
  );
}

/**
 * 视频时间线式分层刻度（自动或指定 zoomMinutes）：
 * - minor：按 zoom
 * - major：1→5分、5→15分、10→30分、30→60分（仅 major 带时间标签）
 * @returns {{ at: Date, major: boolean, label: boolean }[]}
 */
export function buildWindowTicksHierarchical(
  windowStart,
  windowEnd,
  zoomMinutes,
) {
  const zoom =
    zoomMinutes != null
      ? Math.max(1, Number(zoomMinutes) || 1)
      : autoZoomMinutes(windowStart, windowEnd);
  const minorMs = zoom * 60_000;
  const majorMin = zoom <= 1 ? 5 : zoom <= 5 ? 15 : zoom <= 10 ? 30 : 60;
  const start = new Date(windowStart);
  const end = new Date(windowEnd);
  if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime()) || end < start) {
    return [];
  }
  const ticks = [];
  const t0 = Math.floor(start.getTime() / minorMs) * minorMs;
  for (let t = t0; t <= end.getTime() + 1; t += minorMs) {
    if (t < start.getTime() - 1) continue;
    const d = new Date(t);
    const major =
      d.getSeconds() === 0 &&
      d.getMilliseconds() === 0 &&
      d.getMinutes() % majorMin === 0;
    ticks.push({
      at: d,
      major,
      label: major,
    });
  }
  if (ticks.length && !ticks.some((x) => x.label)) {
    ticks[0].label = true;
    ticks[0].major = true;
  }
  return ticks;
}

/**
 * 日期栏标签：窗起点 + 每个自然日 0:00（若落在窗内）
 * @returns {{ at: Date, text: string }[]}
 */
export function buildDateLaneLabels(windowStart, windowEnd) {
  const start = new Date(windowStart);
  const end = new Date(windowEnd);
  if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime()) || end < start) {
    return [];
  }
  const refYear = start.getFullYear();
  const labels = [{ at: start, text: formatDateLane(start, refYear) }];
  const dayMs = 86_400_000;
  let cursor = new Date(start);
  cursor.setHours(0, 0, 0, 0);
  cursor = new Date(cursor.getTime() + dayMs);
  while (cursor.getTime() <= end.getTime()) {
    labels.push({ at: new Date(cursor), text: formatDateLane(cursor, refYear) });
    cursor = new Date(cursor.getTime() + dayMs);
  }
  return labels;
}

export function inTimeRange(ts, startIso, endIso) {
  const t = new Date(ts).getTime();
  if (!Number.isFinite(t)) return false;
  if (startIso) {
    const a = new Date(startIso).getTime();
    if (Number.isFinite(a) && t < a) return false;
  }
  if (endIso) {
    const b = new Date(endIso).getTime();
    if (Number.isFinite(b) && t > b) return false;
  }
  return true;
}

/** 相对 playhead：future | near | faded | outside */
export function playheadVisibility(ts, playheadIso, nearMs = 30 * 60_000) {
  const t = new Date(ts).getTime();
  const p = new Date(playheadIso).getTime();
  if (!Number.isFinite(t) || !Number.isFinite(p)) return "outside";
  if (t > p) return "future";
  if (t > p - nearMs) return "near";
  return "faded";
}

/**
 * @param {string} datasetId
 * @param {{ pageSize?: number, filters?: Record<string, unknown>, signal?: AbortSignal }} [opts]
 * @returns {Promise<{ rows: object[], error?: string }>}
 */
export async function queryThunderDataset(datasetId, opts = {}) {
  const id = String(datasetId || "").trim();
  if (!id) return { rows: [], error: "missing dataset_id" };
  const pageSize = Number(opts.pageSize) > 0 ? Number(opts.pageSize) : 2000;
  const filters =
    opts.filters && typeof opts.filters === "object" ? opts.filters : {};
  const res = await fetch(`/api/datasets/query/${APP_ID}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    cache: "no-store",
    signal: opts.signal,
    body: JSON.stringify({
      dataset_id: id,
      page: 1,
      page_size: pageSize,
      filters,
      query_state: { filters },
      full: true,
      summary: false,
    }),
  });
  const text = await res.text();
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch (_) {
    body = null;
  }
  if (!res.ok) {
    const err =
      (body && (body.error || body.message)) ||
      `datasets/query ${res.status}`;
    throw new Error(String(err));
  }
  const rows = Array.isArray(body?.rows)
    ? body.rows
    : Array.isArray(body?.data?.rows)
      ? body.data.rows
      : Array.isArray(body?.items)
        ? body.items
        : [];
  return { rows, meta: body?.meta || body?.summary || null };
}

export function parseEfieldRefLines(text, fallback = [3, 7, 9]) {
  const parts = String(text || "")
    .split(/[,;\s]+/)
    .map((p) => Number(p))
    .filter((n) => Number.isFinite(n));
  return parts.length ? parts : fallback.slice();
}

export function siteIdsFromMonitorRows(rows) {
  const set = new Set();
  for (const row of rows || []) {
    const id = String(row?.site_id || "").trim();
    if (id) set.add(id);
  }
  return [...set];
}
