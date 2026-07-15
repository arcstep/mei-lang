/**
 * Thunder 产品阈值（释义用参考线，非实时判决 UI）。
 * 口径见 apps/thunder/docs/30-algorithm-derived.md · 20-source-data-schema.md
 */

/** |E| 态势档（kV/m）：活跃 ~3 → 发展 ~7 → 预警 ~9（对应原文 −3/−7/−9） */
export const EFIELD_ABS_THRESHOLDS = [
  {
    key: "yellow",
    value: 3,
    color: "#facc15",
    label: "活跃 |E| ≥ 3 kV/m",
    tag: "3 kV/m",
    title: "活跃档参考：|E| ≈ 3 kV/m（原文 −3）",
  },
  {
    key: "orange",
    value: 7,
    color: "#fb923c",
    label: "发展 |E| ≥ 7 kV/m",
    tag: "7 kV/m",
    title: "发展档参考：|E| ≈ 7 kV/m（原文 −7）",
  },
  {
    key: "red",
    value: 9,
    color: "#f87171",
    label: "预警 |E| ≥ 9 kV/m",
    tag: "9 kV/m",
    title: "预警档参考：|E| ≈ 9 kV/m（原文 −9；B1 默认阈）",
  },
];

/** 定位仪圈内 5min 次数：黄圈 >5 / 橙圈 >3 / 红圈 ≥1（即 >0） */
export const LIGHTNING_FREQ_THRESHOLDS = [
  {
    key: "red",
    value: 1,
    color: "#f87171",
    label: "红圈 ≥1 次 / 5min",
    tag: "≥1次",
    title: "红圈：5 分钟内 1 次即发",
  },
  {
    key: "orange",
    value: 3,
    color: "#fb923c",
    label: "橙圈 >3 次 / 5min",
    tag: ">3次",
    title: "橙圈：5 分钟内 >3 次才发",
  },
  {
    key: "yellow",
    value: 5,
    color: "#facc15",
    label: "黄圈 >5 次 / 5min",
    tag: ">5次",
    title: "黄圈：5 分钟内 >5 次才发",
  },
];

export const EFIELD_ABS_HINT = "阈 |E| 3/7/9 kV/m";
export const LIGHTNING_FREQ_HINT = "圈阈 ≥1次 / >3次 / >5次（5min）";

export function levelCodeColor(code) {
  const n = Number(code);
  if (n >= 3) return "#f87171";
  if (n >= 2) return "#fb923c";
  if (n >= 1) return "#facc15";
  return "#94a3b8";
}

export function levelLabelColor(level) {
  const text = String(level || "");
  if (text.includes("红")) return "#f87171";
  if (text.includes("橙")) return "#fb923c";
  if (text.includes("黄")) return "#facc15";
  return "#94a3b8";
}

/** 展示用 |E| 峰值 */
export function eAbsPeak(event) {
  const raw = Number(event?.e_max);
  if (!Number.isFinite(raw)) return null;
  return Math.abs(raw);
}
