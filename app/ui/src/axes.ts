// Axes, grid and labels on a 2D canvas over the WebGL one (text in WebGL isn't worth it).

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

function timeLabel(tNs: bigint, stepNs: number): string {
  if (tNs < BigInt(RELATIVE_BEFORE) && tNs > -BigInt(RELATIVE_BEFORE)) {
    const s = Number(tNs) / NS;
    const digits = Math.max(0, Math.min(9, Math.ceil(-Math.log10(stepNs / NS))));
    return `${s.toFixed(digits)} s`;
  }
  const ms = Number(tNs / 1_000_000n);
  const d = new Date(ms);
  const pad = (n: number, w = 2) => String(n).padStart(w, '0');
  const hms = `${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}`;
  if (stepNs >= 86400 * NS) return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())}`;
  if (stepNs >= NS) return hms;
  const subNs = Number(((tNs % BigInt(NS)) + BigInt(NS)) % BigInt(NS));
  const digits = Math.max(1, Math.min(9, Math.ceil(-Math.log10(stepNs / NS))));
  return `${hms}.${String(subNs).padStart(9, '0').slice(0, digits)}`;
}

function numberLabel(v: number, step: number): string {
  const digits = Math.max(0, Math.min(8, Math.ceil(-Math.log10(step))));
  return v.toFixed(digits);
}

export interface AxesView {
  t0: bigint;
  t1: bigint;
  yMin: number;
  yMax: number;
}

export const PAD = { left: 56, right: 12, top: 10, bottom: 24 };

export function drawAxes(canvas: HTMLCanvasElement, view: AxesView): void {
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
  }
  const ctx = canvas.getContext('2d')!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  const css = getComputedStyle(document.documentElement);
  const grid = css.getPropertyValue('--grid').trim();
  const muted = css.getPropertyValue('--muted').trim();
  const mono = css.getPropertyValue('--mono').trim();
  const plotW = w - PAD.left - PAD.right;
  const plotH = h - PAD.top - PAD.bottom;
  if (plotW <= 0 || plotH <= 0) return;

  ctx.font = `11px ${mono}`;
  ctx.fillStyle = muted;
  ctx.strokeStyle = grid;
  ctx.lineWidth = 1;

  // time axis
  const span = Number(view.t1 - view.t0);
  const step = TIME_STEPS.find((s) => s >= span / Math.max(2, plotW / 110)) ?? TIME_STEPS[TIME_STEPS.length - 1];
  const stepBig = BigInt(Math.round(step));
  let t = ((view.t0 / stepBig) + (view.t0 % stepBig > 0n ? 1n : 0n)) * stepBig;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'top';
  for (; t < view.t1; t += stepBig) {
    const x = PAD.left + (Number(t - view.t0) / span) * plotW;
    ctx.beginPath();
    ctx.moveTo(Math.round(x) + 0.5, PAD.top);
    ctx.lineTo(Math.round(x) + 0.5, PAD.top + plotH);
    ctx.stroke();
    ctx.fillText(timeLabel(t, step), x, PAD.top + plotH + 6);
  }

  // value axis
  if (Number.isFinite(view.yMin) && Number.isFinite(view.yMax) && view.yMax > view.yMin) {
    const ystep = niceStep(view.yMax - view.yMin, Math.max(2, plotH / 40));
    ctx.textAlign = 'right';
    ctx.textBaseline = 'middle';
    for (let v = Math.ceil(view.yMin / ystep) * ystep; v <= view.yMax; v += ystep) {
      const y = PAD.top + plotH - ((v - view.yMin) / (view.yMax - view.yMin)) * plotH;
      ctx.beginPath();
      ctx.moveTo(PAD.left, Math.round(y) + 0.5);
      ctx.lineTo(PAD.left + plotW, Math.round(y) + 0.5);
      ctx.stroke();
      ctx.fillText(numberLabel(v, ystep), PAD.left - 6, y);
    }
  }
}
