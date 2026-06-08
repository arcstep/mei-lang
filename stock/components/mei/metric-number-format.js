/**
 * 指标数值展示格式：默认 compact（不补零）；可按指标声明 fraction / significant。
 *
 * value_format 示例：
 * - { "fraction_digits": 1 }  → 16.4、12.0
 * - { "fraction_digits": 2 }  → 16.40
 * - { "mode": "significant", "significant_digits": 2 }  → 0.000034 → "0.000034"
 * - { "mode": "compact" }     → 16.4 不变成 16.40
 */

function clampInt(n, min, max) {
  return Math.max(min, Math.min(max, Math.trunc(n)));
}

export function normalizeMetricValueFormat(format) {
  if (format == null || typeof format !== "object") return null;
  const modeRaw = String(format.mode ?? format.style ?? "").trim().toLowerCase();

  const fractionRaw = format.fraction_digits ?? format.fractionDigits ?? null;
  const significantRaw = format.significant_digits ?? format.significantDigits ?? null;
  const precisionRaw = format.precision;

  if (modeRaw === "fraction" || fractionRaw != null || (precisionRaw != null && modeRaw !== "significant")) {
    const digits = Number(fractionRaw ?? precisionRaw);
    return {
      mode: "fraction",
      digits: Number.isFinite(digits) ? clampInt(digits, 0, 8) : 2,
    };
  }
  if (modeRaw === "significant" || significantRaw != null) {
    const digits = Number(significantRaw ?? precisionRaw);
    return {
      mode: "significant",
      digits: Number.isFinite(digits) ? clampInt(digits, 1, 12) : 2,
    };
  }
  if (modeRaw === "compact") {
    return { mode: "compact" };
  }
  return null;
}

function formatSignificantDecimal(n, digits) {
  const prec = n.toPrecision(digits);
  if (!/[eE]/.test(prec)) {
    return String(Number.parseFloat(prec));
  }
  const v = Number(prec);
  if (v === 0) return "0";
  const abs = Math.abs(v);
  const order = Math.floor(Math.log10(abs));
  const fractionDigits = Math.max(0, digits - 1 - order);
  return v.toFixed(Math.min(fractionDigits, 12));
}

export function formatMetricNumber(raw, { unit = "", format = null } = {}) {
  const numeric = Number(raw);
  if (!Number.isFinite(numeric)) {
    return String(raw ?? "");
  }

  let display = numeric;
  const unitText = String(unit ?? "").trim();
  if (unitText === "%" && Math.abs(numeric) <= 1) {
    display = numeric * 100;
  }

  const spec = normalizeMetricValueFormat(format);
  if (!spec || spec.mode === "compact") {
    if (Number.isInteger(display)) {
      return String(display);
    }
    return String(Number.parseFloat(display.toPrecision(12)));
  }
  if (spec.mode === "fraction") {
    return display.toFixed(spec.digits);
  }
  if (spec.mode === "significant") {
    return formatSignificantDecimal(display, spec.digits);
  }
  return String(display);
}
