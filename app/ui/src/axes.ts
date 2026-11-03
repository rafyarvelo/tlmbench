// Axes, grid and labels on a 2D canvas over the WebGL one (text in WebGL isn't worth it), and the time and value
// formats readouts share.

import type { Rect } from './gl';

const NS = 1_000_000_000;
/** Times before 2000 are offsets from the start of a relative recording, not dates. */
const RELATIVE_BEFORE = 946_684_800 * NS;

/** Nice time steps in nanoseconds: 1-2-5 below a second, then clock-friendly steps. */
const TIME_STEPS = (() => {
  const steps: number[] = [];
  for (let e = 0; e < 9; e++) for (const m of [1, 2, 5]) steps.push(m * 10 ** e);
  for (const s of [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 10800, 21600, 43200, 86400])
    steps.push(s * NS);
  for (const d of [2, 7, 14, 30, 60, 91, 182, 365]) steps.push(d * 86400 * NS);
  return steps;
})();

function niceStep(span: number, maxTicks: number): number {
  const raw = span / Math.max(1, maxTicks);
  const p = 10 ** Math.floor(Math.log10(raw));
  for (const m of [1, 2, 5, 10]) if (m * p >= raw) return m * p;
  return 10 * p;
}

const pad = (n: number, w = 2) => String(n).padStart(w, '0');

export type TimeFormat = 'auto' | 'clock' | 's' | 'ms' | 'minsec' | 'h' | 'd';
/** The formats Settings and the range menu offer: the key, its name and an example. */
export const TIME_FORMATS: [TimeFormat, string, string][] = [
  ['auto', 'Automatic', 'clock, dates or seconds, from the file'],
  ['clock', 'Clock, from start', '00:41:12.380'],
  ['s', 'Seconds', '2472.380 s'],
  ['ms', 'Milliseconds', '2472380 ms'],
  ['minsec', 'Minutes and seconds', '41:12.380'],
  ['h', 'Hours', '0.69 h'],
  ['d', 'Days', '0.03 d'],
];

const timeFmt = { mode: 'auto' as TimeFormat, decimals: null as number | null, origin: () => 0n };
/** Sets the time format every label uses: its decimals (null: from the step) and where "from start" counts from. */
export function setTimeFormat(mode: TimeFormat, decimals: number | null, origin: () => bigint): void {
  Object.assign(timeFmt, { mode, decimals, origin });
}

/** A time `rel` ns from the start of the file, in the chosen format. */
function fromStart(rel: bigint, stepNs: number): string {
  const neg = rel < 0n ? '−' : '';
  const a = Number(rel < 0n ? -rel : rel);
  const auto = Math.max(0, Math.min(9, Math.ceil(-Math.log10(stepNs / NS))));
  const d = timeFmt.decimals ?? auto;
  switch (timeFmt.mode) {
    case 's':
      return `${neg}${(a / NS).toFixed(d)} s`;
    case 'ms':
      return `${neg}${(a / 1e6).toFixed(Math.max(0, d - 3))} ms`;
    case 'h':
      return `${neg}${(a / 3.6e12).toFixed(Math.max(1, d - 1))} h`;
    case 'd':
      return `${neg}${(a / 86.4e12).toFixed(Math.max(1, d - 1))} d`;
    default: {
      // clock and minutes-and-seconds: round first, so 59.97 s at one decimal is 1:00.0, not 0:60.0
      const q = 10 ** d;
      const t = d > 0 ? Math.round((a / NS) * q) / q : Math.floor(a / NS);
      const [h, m, s] = [Math.floor(t / 3600), Math.floor((t % 3600) / 60), t % 60];
      const ss = d > 0 ? s.toFixed(d).padStart(d + 3, '0') : pad(Math.floor(s));
      return timeFmt.mode === 'minsec' ? `${neg}${h * 60 + m}:${ss}` : `${neg}${pad(h)}:${pad(m)}:${ss}`;
    }
  }
}

/** A time at the resolution `stepNs` calls for: a clock time for dates, seconds for relative recordings. */
export function timeLabel(tNs: bigint, stepNs: number): string {
  if (timeFmt.mode !== 'auto') return fromStart(tNs - timeFmt.origin(), stepNs);
  if (tNs < BigInt(RELATIVE_BEFORE) && tNs > -BigInt(RELATIVE_BEFORE)) {
    const s = Number(tNs) / NS;
    const digits = Math.max(0, Math.min(9, Math.ceil(-Math.log10(stepNs / NS))));
    return `${s.toFixed(digits)} s`;
  }
  const d = new Date(Number(tNs / 1_000_000n));
  const hms = `${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}`;
  if (stepNs >= 86400 * NS) return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}`;
  if (stepNs >= NS) return hms;
  const subNs = Number(((tNs % BigInt(NS)) + BigInt(NS)) % BigInt(NS));
  const digits = Math.max(1, Math.min(9, Math.ceil(-Math.log10(stepNs / NS))));
  return `${hms}.${String(subNs).padStart(9, '0').slice(0, digits)}`;
}

/** A time for a readout, at a thousandth of the visible span. */
export function readoutTime(tNs: bigint, spanNs: number): string {
  return timeLabel(tNs, Math.max(1, spanNs / 1000));
}

/** A duration: `250 µs`, `1.25 s`, `00:03:20.5`. */
export function duration(ns: number): string {
  const a = Math.abs(ns);
  const sign = ns < 0 ? '−' : '';
  if (a < 1e3) return `${sign}${a.toFixed(0)} ns`;
  if (a < 1e6) return `${sign}${(a / 1e3).toPrecision(4)} µs`;
  if (a < 1e9) return `${sign}${(a / 1e6).toPrecision(4)} ms`;
  if (a < 60e9) return `${sign}${(a / 1e9).toPrecision(5)} s`;
  const s = a / 1e9;
  const tail = (s % 60).toFixed(1).padStart(4, '0');
  return `${sign}${pad(Math.floor(s / 3600))}:${pad(Math.floor(s / 60) % 60)}:${tail}`;
}

/** How much time the view spans, in the unit that suits it: `2.00 h`, `3.33 min`, `10.00 s`, `250 ms`. */
export function spanText(ns: number): string {
  const s = ns / NS;
  if (s >= 3600) return `${(s / 3600).toFixed(2)} h`;
  if (s >= 60) return `${(s / 60).toFixed(2)} min`;
  if (s >= 1) return `${s.toFixed(2)} s`;
  if (ns >= 1e6) return `${(ns / 1e6).toPrecision(3)} ms`;
  if (ns >= 1e3) return `${(ns / 1e3).toPrecision(3)} µs`;
  return `${Math.round(ns)} ns`;
}

/** A value with about six significant digits, as readouts show it. */
export function value(v: number | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return '—';
  const a = Math.abs(v);
  if (a !== 0 && (a >= 1e7 || a < 1e-4)) return v.toExponential(4);
  return String(Number(v.toPrecision(6)));
}

/** A frequency: `12.5 Hz`, `1.2 kHz`. */
export function frequency(hz: number): string {
  if (!Number.isFinite(hz)) return '—';
  if (hz >= 1e6) return `${(hz / 1e6).toPrecision(4)} MHz`;
  if (hz >= 1e3) return `${(hz / 1e3).toPrecision(4)} kHz`;
  return `${hz.toPrecision(4)} Hz`;
}

function numberLabel(v: number, step: number): string {
  const digits = Math.max(0, Math.min(8, Math.ceil(-Math.log10(step))));
  return v.toFixed(digits);
}

/** A value axis as drawn: its range (in log10 units for a log axis), decimals and label. */
export interface YAxis {
  min: number;
  max: number;
  log?: boolean;
  dec?: number;
  label?: string;
}

export interface AxesView {
  t0: bigint;
  t1: bigint;
  left: YAxis;
  /** A second unit's axis, labelled on the right. */
  right?: YAxis;
}

/** A value axis's ticks: positions in the axis's units (log10 for a log axis) and their labels. */
function yTicks(ax: YAxis, h: number): [number, string][] {
  if (!(Number.isFinite(ax.min) && Number.isFinite(ax.max) && ax.max > ax.min)) return [];
  const label = (v: number, step: number) => (ax.dec != null ? v.toFixed(ax.dec) : numberLabel(v, step));
  if (ax.log && ax.max - ax.min >= 1) {
    // whole decades, every k-th when there are many
    const k = Math.ceil((ax.max - ax.min) / Math.max(2, h / 28));
    const out: [number, string][] = [];
    for (let e = Math.ceil(ax.min / k) * k; e <= ax.max; e += k) out.push([e, ax.dec != null ? (10 ** e).toFixed(ax.dec) : value(10 ** e)]);
    return out;
  }
  if (ax.log) {
    // under a decade: linear steps between the ends
    const [lo, hi] = [10 ** ax.min, 10 ** ax.max];
    const step = niceStep(hi - lo, Math.max(2, h / 28));
    const out: [number, string][] = [];
    for (let v = Math.ceil(lo / step) * step; v <= hi; v += step) if (v > 0) out.push([Math.log10(v), label(v, step)]);
    return out;
  }
  const step = niceStep(ax.max - ax.min, Math.max(2, h / 28));
  const out: [number, string][] = [];
  for (let v = Math.ceil(ax.min / step) * step; v <= ax.max; v += step) out.push([v, label(v, step)]);
  return out;
}

export interface AxesStyle {
  grid: string;
  muted: string;
  mono: string;
  /** Draw the time labels under the plot (the last pane only). */
  timeLabels: boolean;
}

/** Margins around each pane's plot for the value labels on the left and the time labels under the last one. */
export const PAD = { left: 52, right: 14, timeLabels: 22 };

/** Grid lines and labels for the plot in `r` (CSS pixels on the overlay). */
export function drawAxes(ctx: CanvasRenderingContext2D, r: Rect, view: AxesView, s: AxesStyle): void {
  if (r.w <= 0 || r.h <= 0) return;
  ctx.save();
  ctx.font = `11px ${s.mono}`;
  ctx.fillStyle = s.muted;
  ctx.strokeStyle = s.grid;
  ctx.lineWidth = 1;

  const span = Number(view.t1 - view.t0);
  const step = TIME_STEPS.find((x) => x >= span / Math.max(2, r.w / 110)) ?? TIME_STEPS[TIME_STEPS.length - 1];
  const stepBig = BigInt(Math.round(step));
  let t = (view.t0 / stepBig + (view.t0 % stepBig > 0n ? 1n : 0n)) * stepBig;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'top';
  for (; t < view.t1; t += stepBig) {
    const x = Math.round(r.x + (Number(t - view.t0) / span) * r.w) + 0.5;
    ctx.beginPath();
    ctx.moveTo(x, r.y);
    ctx.lineTo(x, r.y + r.h);
    ctx.stroke();
    if (s.timeLabels) ctx.fillText(timeLabel(t, step), x, r.y + r.h + 6);
  }

  const yAt = (ax: YAxis, v: number) => Math.round(r.y + r.h - ((v - ax.min) / (ax.max - ax.min)) * r.h) + 0.5;
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'right';
  for (const [v, text] of yTicks(view.left, r.h)) {
    const y = yAt(view.left, v);
    ctx.beginPath();
    ctx.moveTo(r.x, y);
    ctx.lineTo(r.x + r.w, y);
    ctx.stroke();
    ctx.fillText(text, r.x - 8, y);
  }
  if (view.right) {
    ctx.textAlign = 'left';
    for (const [v, text] of yTicks(view.right, r.h)) ctx.fillText(text, r.x + r.w + 8, yAt(view.right, v));
  }
  // axis labels sit inside the plot's top corners, so plot areas stay aligned across panes
  ctx.textBaseline = 'top';
  if (view.left.label) {
    ctx.textAlign = 'left';
    ctx.fillText(view.left.label, r.x + 6, r.y + 4);
  }
  if (view.right?.label) {
    ctx.textAlign = 'right';
    ctx.fillText(view.right.label, r.x + r.w - 6, r.y + 4);
  }
  ctx.restore();
}
