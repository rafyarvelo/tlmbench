// Phase 2 UI: a stack of panes on one time axis, an overview of the whole recording, hover readouts and cursors
// with Measure. Pan and zoom re-project the last reply at once (two uniforms per trace) while the core computes
// the sharper one; replies carry a generation and older ones are dropped. At most one query is in flight, and
// the latest view is asked for as soon as it returns. One query asks for every pane (each at its own width) and,
// when it needs redrawing, the overview, so the stack redraws together.
//
// On a cold file the first reply is coarse (from whole summaries, labelled), and the exact one follows. While the
// pyramid is built, panes fill in left to right and the part not read yet is hatched.

import { Channel, invoke } from '@tauri-apps/api/core';
import { tick } from 'svelte';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

import { type AxesStyle, drawAxes, duration, frequency, PAD, readoutTime, value } from './axes';
import { runGate, runGate2 } from './gate';
import { type Axis, type GlTrace, type Rect, Renderer, type Rgba } from './gl';
import { applyTheme, type Mode, onSetting, saveSettings, setSetting, settings } from './settings.svelte';
import { addPaneBelow, labelled, normalize, note, onStackChange, type Pane, stack } from './stack.svelte';
import { actions, type FolderEntry, ui } from './ui.svelte';
import { type Batch, decodeBatch, type Reply } from './wire';

interface ChannelInfo {
  name: string;
  unit: string | null;
  dtype: string;
}

export interface RecordingInfo {
  path: string;
  format: string;
  samples: number;
  /** Where sample times come from: a column has gaps; a fixed rate or row numbers don't. */
  time: { kind: 'column'; name: string } | { kind: 'rate'; hz: number } | { kind: 'index' };
  channels: ChannelInfo[];
  skipped: string[];
}

type Status = { state: 'missing' } | { state: 'building'; done: number; total: number } | { state: 'built' };

interface Opened {
  id: number;
  info: RecordingInfo;
  /** First and last sample times as decimal strings (exact i64 nanoseconds). */
  range: [string, string] | null;
  status: Status;
}

interface Progress {
  done: number;
  total: number;
  builtUntil: string | null;
  error: string | null;
}

interface Nearest {
  channel: number;
  index: number;
  t: string;
  value: number | null;
}

interface ChannelStats {
  channel: number;
  count: number;
  min: number | null;
  minT: string | null;
  max: number | null;
  maxT: string | null;
  mean: number | null;
  std: number | null;
}

export interface QueryStats {
  /** Round trip, as the page's clock sees it (1 ms resolution in WebKit). */
  ms: number;
  /** The core's time for the request, measured in Rust. */
  reduceUs: number;
  points: number;
  bytes: number;
  samplesInView: number;
  coarse: boolean;
  traces: number;
}

/** A reply's traces on the GPU, with what's needed to place them. */
interface Layer {
  originNs: bigint;
  xScaleNs: number;
  /** Samples after this time weren't summarized when it was drawn (null: all were). */
  builtUntil: bigint | null;
  traces: Map<number, { gl: GlTrace; yMin: number; yMax: number; coarse: boolean; points: number }>;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const center = $<HTMLDivElement>('center');
const stackEl = $<HTMLDivElement>('stack');
const overlay = $<HTMLCanvasElement>('overlay');
const measureEl = $<HTMLElement>('measure');
const renderer = new Renderer($<HTMLCanvasElement>('gl'));

// ---------- state ----------

const state = {
  file: null as null | { id: number; info: RecordingInfo; first: bigint; last: bigint },
  t0: 0n,
  t1: 1n,
  history: [] as [bigint, bigint][],
  future: [] as [bigint, bigint][],
  /** Each pane's last reply. */
  layers: [] as (Layer | null)[],
  ov: null as Layer | null,
  /** The overview needs asking for again (layout, selection, size or build progress changed, or it's coarse). */
  ovStale: true,
  generation: 0,
  drawnGeneration: 0,
  inflight: false,
  pending: false,
  /** The last exact reply's round trip: a slow one means a cold file, so new views ask for coarse first. */
  exactMs: Infinity,
  /** Build progress; null when built. */
  building: null as null | { done: number; total: number; until: bigint | null },
  /** Where the hatching starts on screen: it glides toward what the panes have drawn rather than jumping. */
  hatchFrom: null as bigint | null,
  /** When each pane's view became coarse: the label shows only if it stays coarse a moment. */
  coarseSince: [] as (number | null)[],
  hoverT: null as bigint | null,
  hover: new Map<number, { t: bigint; v: number | null }>(),
  A: null as bigint | null,
  B: null as bigint | null,
  box: null as null | { pane: number; x0: number; x1: number },
  drawScheduled: false,
  onQuery: null as null | ((q: QueryStats) => void),
  onPaint: null as null | ((p: { coarse: boolean; complete: boolean }) => void),
};

// ---------- colours ----------

let tokenCache: Record<string, string> | null = null;
function tokens(): Record<string, string> {
  if (tokenCache) return tokenCache;
  const css = getComputedStyle(document.documentElement);
  const names = ['--plot-bg', '--grid', '--fg', '--fg-muted', '--accent', '--cursor', '--border', '--font-data', '--warn'];
  for (let i = 1; i <= 8; i++) names.push(`--trace-${i}`);
  tokenCache = Object.fromEntries(names.map((n) => [n, css.getPropertyValue(n).trim()]));
  return tokenCache;
}
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  tokenCache = null;
  renderPanes();
});
const colorOf = (ch: number) => tokens()[`--trace-${(ch % 8) + 1}`];
function rgba(hex: string, a = 1): Rgba {
  const h = hex.replace('#', '');
  const n = parseInt(h.length === 3 ? [...h].map((c) => c + c).join('') : h, 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255, a];
}
const css = (hex: string, a: number) => {
  const [r, g, b] = rgba(hex);
  return `rgba(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)},${a})`;
};

// ---------- geometry ----------

function rectIn(el: Element, inset: { l: number; r: number; t: number; b: number }): Rect {
  const c = center.getBoundingClientRect();
  const r = el.getBoundingClientRect();
  return { x: r.left - c.left + inset.l, y: r.top - c.top + inset.t, w: r.width - inset.l - inset.r, h: r.height - inset.t - inset.b };
}

/** The plot rectangle of pane `i`, in CSS pixels on the canvases. */
function plotRect(i: number): Rect {
  const el = stackEl.querySelector(`[data-pane="${i}"] .plot`);
  if (!el) return { x: 0, y: 0, w: 0, h: 0 };
  return rectIn(el, { l: PAD.left, r: PAD.right, t: 4, b: labelled(i) ? PAD.timeLabels : 4 });
}

function ovRect(): Rect | null {
  const el = document.querySelector('.ovplot');
  return el ? rectIn(el, { l: 1, r: 1, t: 1, b: 1 }) : null;
}

/** Pixel columns the core reduces pane `i` for: its plot width in device pixels. */
function widthPx(i = 0): number {
  return Math.max(1, Math.round(plotRect(i).w * (window.devicePixelRatio || 1)));
}

/** The overview's entry id in a view batch (panes are 0, 1, …). */
const OVERVIEW = 0xffffffff;

/** The default gap factor, and the smallest a channel can have (as in the core). */
const GAP_DEFAULT = 4;
const GAP_MIN = 2;

/** The gap factor of channel `ch`: a time jump over this many median intervals breaks its line. */
const gapFactor = (ch: number): number => (state.file && settings.gapFactors[state.file.info.path]?.[ch]) || GAP_DEFAULT;

/** Each pane's channels. */
const panes = (): number[][] => stack.panes.map((p) => p.channels);
const plotted = () => [...new Set(panes().flat())];

function samplesInView(): number {
  const f = state.file;
  if (!f || f.last <= f.first) return f?.info.samples ?? 0;
  const lo = state.t0 > f.first ? state.t0 : f.first;
  const hi = state.t1 < f.last ? state.t1 : f.last;
  return hi <= lo ? 0 : Math.round((f.info.samples * Number(hi - lo)) / Number(f.last - f.first));
}

const tAt = (r: Rect, x: number) => state.t0 + BigInt(Math.round(((x - r.x) / r.w) * Number(state.t1 - state.t0)));
const xAt = (r: Rect, t: bigint) => r.x + (Number(t - state.t0) / Number(state.t1 - state.t0)) * r.w;

// ---------- queries ----------

function upload(layer: Layer | null, reply: Reply): Layer {
  const next: Layer = { originNs: reply.originNs, xScaleNs: reply.xScaleNs, builtUntil: reply.builtUntil, traces: new Map() };
  for (const t of reply.traces) {
    const old = layer?.traces.get(t.channel);
    layer?.traces.delete(t.channel);
    next.traces.set(t.channel, {
      gl: renderer.upload(t.x, t.y, old?.gl),
      yMin: t.yMin,
      yMax: t.yMax,
      coarse: t.coarse,
      points: t.x.length,
    });
  }
  layer?.traces.forEach((t) => renderer.free(t.gl));
  return next;
}

/** A pane's or the overview's request: its channels with their gap factors, over its own width. */
function entry(id: number, channels: number[], t0: bigint, t1: bigint, width: number) {
  return { id, channels, gapFactors: channels.map(gapFactor), t0: t0.toString(), t1: t1.toString(), widthPx: width };
}

/** Asks for every pane and, when stale, the overview, in one query under one generation. */
async function requestView(precision?: 'coarse' | 'exact'): Promise<void> {
  const f = state.file;
  if (!f || !stack.panes.length) return;
  if (state.inflight) {
    state.pending = true;
    return;
  }
  state.inflight = true;
  state.pending = false;
  const generation = ++state.generation;
  const want = precision ?? (state.exactMs > 150 ? 'coarse' : 'exact');
  const inView = samplesInView();
  const entries = panes().map((chans, i) => entry(i, chans, state.t0, state.t1, widthPx(i)));
  const r = ovRect();
  const withOv = state.ovStale && !!r;
  if (withOv && r) {
    const w = Math.max(1, Math.round(r.w * (window.devicePixelRatio || 1)));
    entries.push(entry(OVERVIEW, panes()[stack.sel] ?? [], f.first, f.last + 1n, w));
    state.ovStale = false;
  }
  const started = performance.now();
  let followUp = false;
  try {
    const batch: Batch = decodeBatch(await invoke<ArrayBuffer>('view', { file: f.id, generation, precision: want, entries }));
    const ms = performance.now() - started;
    const panes = [...batch.entries].filter(([id]) => id !== OVERVIEW).map(([, reply]) => reply);
    const traces = panes.flatMap((reply) => reply.traces);
    const coarse = traces.some((t) => t.coarse);
    if (!coarse) state.exactMs = ms;
    const ov = batch.entries.get(OVERVIEW);
    if (ov) {
      state.ov = upload(state.ov, ov);
      if (ov.traces.some((t) => t.coarse)) state.ovStale = true;
    }
    if (generation > state.drawnGeneration) {
      state.drawnGeneration = generation;
      state.layers = stack.panes.map((_, i) => {
        const reply = batch.entries.get(i);
        return reply ? upload(state.layers[i] ?? null, reply) : (state.layers[i] ?? null);
      });
      const points = traces.reduce((a, t) => a + t.x.length, 0);
      const kind = traces.some((t) => t.kind === 'm4') ? 'M4' : 'raw';
      ui.perf = `${points.toLocaleString()} points · ${kind}${coarse ? ' · coarse' : ''} · ${ms.toFixed(0)} ms`;
      state.onQuery?.({ ms, reduceUs: traces[0]?.reduceUs ?? 0, points, bytes: batch.bytes, samplesInView: inView, coarse, traces: traces.length });
      followUp = coarse && !state.pending;
    }
    if (ov && state.ovStale && !state.pending) followUp = true;
    scheduleDraw();
  } catch (e) {
    if (withOv) state.ovStale = true;
    note(`Couldn't draw the view: ${e}`);
  } finally {
    state.inflight = false;
    if (state.pending) void requestView();
    else if (followUp) void requestView('exact');
  }
}

/** Asks for the overview again with the panes. */
function overviewChanged(): void {
  state.ovStale = true;
  void requestView();
}

// hover: each channel's real sample nearest the pointer
let hoverInflight = false;
let hoverWanted: bigint | null = null;
async function requestHover(t: bigint | null): Promise<void> {
  hoverWanted = t;
  if (hoverInflight || !state.file) return;
  if (t == null) {
    state.hover.clear();
    updateChips();
    scheduleDraw();
    return;
  }
  hoverInflight = true;
  try {
    const got = await invoke<Nearest[]>('values_at', { file: state.file.id, channels: plotted(), t: t.toString() });
    if (hoverWanted != null) {
      state.hover = new Map(got.map((n) => [n.channel, { t: BigInt(n.t), v: n.value }]));
      updateChips();
      scheduleDraw();
    }
  } finally {
    hoverInflight = false;
    if (hoverWanted !== t) void requestHover(hoverWanted);
  }
}

// ---------- view ----------

function setView(t0: bigint, t1: bigint, remember = false): void {
  const f = state.file;
  if (!f) return;
  if (remember) {
    state.history.push([state.t0, state.t1]);
    state.future = [];
  }
  const full = f.last - f.first || 1n;
  let span = t1 - t0;
  const minSpan = 1000n; // 1 µs
  const maxSpan = full * 3n;
  if (span < minSpan || span > maxSpan) {
    const mid = t0 + span / 2n;
    span = span < minSpan ? minSpan : maxSpan;
    t0 = mid - span / 2n;
  }
  state.t0 = t0;
  state.t1 = t0 + span;
  updateRange();
  scheduleDraw();
  void requestView();
}

/** Glides to a view over about 0.3 s on its centre and log(span), so a 2× and a 200× zoom look even. */
let glide = 0;
function glideTo(t0: bigint, t1: bigint): void {
  state.history.push([state.t0, state.t1]);
  state.future = [];
  const id = ++glide;
  if (matchMedia('(prefers-reduced-motion: reduce)').matches) return setView(t0, t1);
  const [c0, s0] = [Number(state.t0 + state.t1) / 2, Math.log(Number(state.t1 - state.t0))];
  const [c1, s1] = [Number(t0 + t1) / 2, Math.log(Number(t1 - t0))];
  const start = performance.now();
  const step = (now: number) => {
    if (id !== glide) return;
    const p = Math.min(1, (now - start) / 300);
    const e = 1 - (1 + 8 * p) * Math.exp(-8 * p); // critically damped
    const c = c0 + (c1 - c0) * e;
    const s = Math.exp(s0 + (s1 - s0) * e);
    if (p >= 1) setView(t0, t1);
    else {
      setView(BigInt(Math.round(c - s / 2)), BigInt(Math.round(c + s / 2)));
      requestAnimationFrame(step);
    }
  };
  requestAnimationFrame(step);
}
const stopGlide = () => void glide++;

function fit(): void {
  const f = state.file;
  if (f) glideTo(f.first, f.last + 1n);
}

function back(): void {
  const v = state.history.pop();
  if (!v) return;
  state.future.push([state.t0, state.t1]);
  stopGlide();
  setView(v[0], v[1]);
}

function forward(): void {
  const v = state.future.pop();
  if (!v) return;
  state.history.push([state.t0, state.t1]);
  stopGlide();
  setView(v[0], v[1]);
}

function updateRange(): void {
  const span = Number(state.t1 - state.t0);
  ui.range = state.file ? `${readoutTime(state.t0, span)} → ${readoutTime(state.t1, span)}` : '';
  ui.span = state.file ? `${duration(span)} visible` : '';
  ui.canBack = state.history.length > 0;
}

// ---------- drawing ----------

function draw(): void {
  state.drawScheduled = false;
  renderer.begin();
  const dpr = window.devicePixelRatio || 1;
  const w = overlay.clientWidth;
  const h = overlay.clientHeight;
  if (overlay.width !== Math.round(w * dpr) || overlay.height !== Math.round(h * dpr)) {
    overlay.width = Math.round(w * dpr);
    overlay.height = Math.round(h * dpr);
  }
  const ctx = overlay.getContext('2d')!;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  const f = state.file;
  if (!f) return;
  const tk = tokens();
  const span = Number(state.t1 - state.t0);
  const style: AxesStyle = { grid: tk['--grid'], muted: tk['--fg-muted'], mono: tk['--font-data'], timeLabels: false };
  let coarse = false;
  let complete = stack.panes.length > 0;

  panes().forEach((chans, i) => {
    const r = plotRect(i);
    const main = state.layers[i];
    // the pane's value range: what its traces hold in view
    let lo = Infinity;
    let hi = -Infinity;
    for (const ch of chans) {
      const t = main?.traces.get(ch);
      if (!t) {
        complete = false;
        continue;
      }
      coarse ||= t.coarse;
      if (Number.isFinite(t.yMin)) lo = Math.min(lo, t.yMin);
      if (Number.isFinite(t.yMax)) hi = Math.max(hi, t.yMax);
    }
    if (!(hi >= lo)) [lo, hi] = [-1, 1];
    const padY = (hi - lo) * 0.06 || Math.abs(hi) * 0.06 || 1;
    const [yMin, yMax] = [lo - padY, hi + padY];
    paneY[i] = { yMin, yMax };
    drawAxes(ctx, r, { t0: state.t0, t1: state.t1, yMin, yMax }, { ...style, timeLabels: labelled(i) });
    if (main && span > 0) {
      const x: Axis = { a: (2 * main.xScaleNs) / span, b: (2 * Number(main.originNs - state.t0)) / span - 1 };
      const y: Axis = { a: 2 / (yMax - yMin), b: (-2 * yMin) / (yMax - yMin) - 1 };
      for (const ch of chans) {
        const t = main.traces.get(ch);
        if (t) renderer.draw(t.gl, x, y, rgba(colorOf(ch)), r);
      }
    }
    drawBuilding(ctx, r, i === 0);
    drawCursors(ctx, r, tk);
    drawHover(ctx, r, chans, { yMin, yMax }, tk);
    if (state.box?.pane === i) {
      const [a, b] = [Math.min(state.box.x0, state.box.x1), Math.max(state.box.x0, state.box.x1)];
      ctx.fillStyle = css(tk['--accent'], b - a >= 4 ? 0.16 : 0.07);
      ctx.fillRect(a, r.y, b - a, r.h);
      ctx.strokeStyle = css(tk['--accent'], b - a >= 4 ? 0.8 : 0.35);
      ctx.strokeRect(Math.round(a) + 0.5, r.y + 0.5, Math.round(b - a), r.h - 1);
    }
  });
  // "coarse" shows only when a coarse view stays up (a cold first paint), not when the exact one follows at once
  const now = performance.now();
  const coarsePanes = panes().map((chans, i) => {
    const coarse = chans.some((ch) => state.layers[i]?.traces.get(ch)?.coarse);
    state.coarseSince[i] = coarse ? (state.coarseSince[i] ?? now) : null;
    const since = state.coarseSince[i];
    if (since != null && now - since < COARSE_LABEL_AFTER_MS) setTimeout(scheduleDraw, COARSE_LABEL_AFTER_MS - (now - since) + 5);
    return since != null && now - since >= COARSE_LABEL_AFTER_MS;
  });
  if (coarsePanes.join() !== stack.coarse.join()) stack.coarse = coarsePanes;
  glideHatch();
  drawOverview(ctx, tk);
  state.onPaint?.({ coarse, complete });
}

const paneY: { yMin: number; yMax: number }[] = [];

/** A coarse view replaced sooner than this isn't labelled. */
const COARSE_LABEL_AFTER_MS = 300;

/** Moves the hatching's start a step toward what the panes have drawn (their replies' built-until), each frame. */
function glideHatch(): void {
  if (!state.building) {
    state.hatchFrom = null;
    return;
  }
  const drawn = state.layers.map((l) => l?.builtUntil).filter((t): t is bigint => t != null);
  const target = drawn.length ? drawn.reduce((a, b) => (b < a ? b : a)) : null;
  if (target == null) return;
  const from = state.hatchFrom ?? target;
  if (from >= target) {
    state.hatchFrom = target;
    return;
  }
  const step = (target - from) / 5n;
  state.hatchFrom = step > 0n ? from + step : target;
  scheduleDraw();
}

/** Hatches the part of the view not summarized yet. */
function drawBuilding(ctx: CanvasRenderingContext2D, r: Rect, label: boolean, t0 = state.t0, t1 = state.t1): void {
  const b = state.building;
  if (!b) return;
  const until = state.hatchFrom ?? b.until ?? t0;
  if (until >= t1) return;
  const x0 = Math.max(r.x, r.x + (Number(until - t0) / Number(t1 - t0)) * r.w);
  const tk = tokens();
  ctx.save();
  ctx.beginPath();
  ctx.rect(x0, r.y, r.x + r.w - x0, r.h);
  ctx.clip();
  ctx.strokeStyle = css(tk['--fg-muted'], 0.14);
  for (let x = x0 - r.h; x < r.x + r.w; x += 8) {
    ctx.beginPath();
    ctx.moveTo(x, r.y + r.h);
    ctx.lineTo(x + r.h, r.y);
    ctx.stroke();
  }
  if (label && r.x + r.w - x0 > 120) {
    ctx.fillStyle = tk['--fg-muted'];
    ctx.font = `12px ${getComputedStyle(document.body).fontFamily}`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(`Reading ${Math.floor((100 * b.done) / Math.max(1, b.total))}%`, (x0 + r.x + r.w) / 2, r.y + r.h / 2);
  }
  ctx.restore();
}

function drawCursors(ctx: CanvasRenderingContext2D, r: Rect, tk: Record<string, string>): void {
  ctx.save();
  ctx.font = `500 11px ${tk['--font-data']}`;
  for (const [name, t] of [
    ['A', state.A],
    ['B', state.B],
  ] as const) {
    if (t == null || t < state.t0 || t > state.t1) continue;
    const x = Math.round(xAt(r, t)) + 0.5;
    ctx.strokeStyle = tk['--cursor'];
    ctx.setLineDash([4, 4]);
    ctx.beginPath();
    ctx.moveTo(x, r.y);
    ctx.lineTo(x, r.y + r.h);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = tk['--cursor'];
    ctx.fillRect(x - 7, r.y, 14, 15);
    ctx.fillStyle = tk['--plot-bg'];
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(name, x, r.y + 7.5);
  }
  ctx.restore();
}

/** The pointer's time across the pane, and a dot on each channel's nearest real sample. */
function drawHover(ctx: CanvasRenderingContext2D, r: Rect, chans: number[], y: { yMin: number; yMax: number }, tk: Record<string, string>): void {
  if (state.hoverT == null || state.hoverT < state.t0 || state.hoverT > state.t1) return;
  const x = Math.round(xAt(r, state.hoverT)) + 0.5;
  ctx.strokeStyle = css(tk['--fg-muted'], 0.5);
  ctx.beginPath();
  ctx.moveTo(x, r.y);
  ctx.lineTo(x, r.y + r.h);
  ctx.stroke();
  for (const ch of chans) {
    const s = state.hover.get(ch);
    if (!s || s.v == null || s.t < state.t0 || s.t > state.t1) continue;
    const px = xAt(r, s.t);
    const py = r.y + r.h - ((s.v - y.yMin) / (y.yMax - y.yMin)) * r.h;
    if (py < r.y || py > r.y + r.h) continue;
    ctx.fillStyle = tk['--plot-bg'];
    ctx.strokeStyle = colorOf(ch);
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.arc(px, py, 3.5, 0, 2 * Math.PI);
    ctx.fill();
    ctx.stroke();
    ctx.lineWidth = 1;
  }
}

function drawOverview(ctx: CanvasRenderingContext2D, tk: Record<string, string>): void {
  const f = state.file;
  const r = ovRect();
  if (!f || !r) return;
  const [t0, t1] = [f.first, f.last + 1n];
  const span = Number(t1 - t0);
  const ov = state.ov;
  if (ov && span > 0) {
    const x: Axis = { a: (2 * ov.xScaleNs) / span, b: (2 * Number(ov.originNs - t0)) / span - 1 };
    for (const [ch, t] of ov.traces) {
      if (!(t.yMax >= t.yMin)) continue;
      const pad = (t.yMax - t.yMin) * 0.08 || 1;
      const [lo, hi] = [t.yMin - pad, t.yMax + pad];
      renderer.draw(t.gl, x, { a: 2 / (hi - lo), b: (-2 * lo) / (hi - lo) - 1 }, rgba(colorOf(ch), 0.8), r);
    }
  }
  drawBuilding(ctx, r, false, t0, t1);
  // the window on the current view
  const xa = Math.max(r.x, r.x + (Number(state.t0 - t0) / span) * r.w);
  const xb = Math.min(r.x + r.w, r.x + (Number(state.t1 - t0) / span) * r.w);
  ctx.fillStyle = css(tk['--plot-bg'], 0.55);
  ctx.fillRect(r.x, r.y, Math.max(0, xa - r.x), r.h);
  ctx.fillRect(xb, r.y, Math.max(0, r.x + r.w - xb), r.h);
  ctx.strokeStyle = tk['--accent'];
  ctx.strokeRect(Math.round(xa) + 0.5, r.y + 0.5, Math.max(2, Math.round(xb - xa) - 1), r.h - 1);
  for (const t of [state.A, state.B]) {
    if (t == null) continue;
    const x = Math.round(r.x + (Number(t - t0) / span) * r.w) + 0.5;
    ctx.strokeStyle = tk['--cursor'];
    ctx.beginPath();
    ctx.moveTo(x, r.y);
    ctx.lineTo(x, r.y + r.h);
    ctx.stroke();
  }
}

function scheduleDraw(): void {
  if (state.drawScheduled) return;
  state.drawScheduled = true;
  requestAnimationFrame(draw);
}

// ---------- panes, chips and the channel list ----------

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!);
}

/** What changes with the panes: the channel list, the overview's title, the plots. */
function renderPanes(): void {
  renderChannels();
  renderOverviewHead();
  applyTips();
  scheduleDraw();
}

function renderChannels(): void {
  $('addPane').hidden = !state.file;
}

function renderOverviewHead(): void {
  const slot = settings.overviewTop ? $('ovTop') : $('ovBottom');
  $('ovTop').replaceChildren();
  $('ovBottom').replaceChildren();
  if (!state.file) return;
  const wrap = document.createElement('div');
  wrap.className = 'overview';
  wrap.innerHTML = `<div class="ovhead"><span>Overview of pane ${stack.sel + 1}</span><span class="spacer"></span><button class="linkbtn" data-ovtop>${settings.overviewTop ? 'Show at the bottom' : 'Show on top'}</button></div><div class="ovplot"></div>`;
  slot.append(wrap);
}

function updateChips(): void {
  const values: Record<number, string> = {};
  state.hover.forEach((s, ch) => (values[ch] = value(s.v)));
  if (JSON.stringify(values) !== JSON.stringify(stack.values)) stack.values = values;
  ui.pointer = state.hoverT != null ? readoutTime(state.hoverT, Number(state.t1 - state.t0)) : '';
}

function saveLayout(): void {
  if (!state.file) return;
  settings.layouts[state.file.info.path] = stack.panes.map((p) => ({ ...p, channels: [...p.channels] }));
  saveSettings();
}

/** After an edit of the panes, once they are on the page (their plots are measured). */
async function layoutChanged(): Promise<void> {
  saveLayout();
  await tick();
  renderPanes();
  overviewChanged();
  if (state.A != null && state.B != null) void refreshMeasure();
}

function selectPane(i: number): void {
  if (i === stack.sel) return;
  stack.sel = i;
  renderChannels();
  renderOverviewHead();
  overviewChanged();
}

/** A new file's panes: one per channel up to four, else the channels shared out over four. */
function defaultPanes(n: number): Pane[] {
  if (n <= 4) return Array.from({ length: n }, (_, i) => ({ channels: [i] }));
  const per = Math.ceil(n / 4);
  return Array.from({ length: Math.ceil(n / per) }, (_, p) => ({
    channels: Array.from({ length: Math.min(per, n - p * per) }, (_, k) => p * per + k),
  }));
}

// ---------- Measure ----------

let measureSeq = 0;
async function refreshMeasure(): Promise<void> {
  const f = state.file;
  const seq = ++measureSeq;
  ui.anyCursor = state.A != null || state.B != null;
  ui.cursors = state.A != null && state.B != null;
  if (!f || state.A == null || state.B == null) {
    measureEl.innerHTML = `<h2>Measure</h2><p class="mnote">${f ? 'Place cursors A and B to measure between them: press C, then click a chart twice. A and B also place them at the pointer.' : 'Open a recording to measure it.'}</p>`;
    return;
  }
  const [a, b] = state.A <= state.B ? [state.A, state.B] : [state.B, state.A];
  const chans = plotted();
  const [va, vb, st] = await Promise.all([
    invoke<Nearest[]>('values_at', { file: f.id, channels: chans, t: state.A.toString() }),
    invoke<Nearest[]>('values_at', { file: f.id, channels: chans, t: state.B.toString() }),
    invoke<ChannelStats[] | null>('stats', { file: f.id, channels: chans, span: { t0: a.toString(), t1: (b + 1n).toString() } }),
  ]);
  if (seq !== measureSeq) return;
  const span = Number(state.t1 - state.t0);
  const dt = Number(state.B - state.A);
  const at = (list: Nearest[], ch: number) => list.find((n) => n.channel === ch)?.value ?? null;
  const rows = (pairs: [string, string][]) => pairs.map(([k, v]) => `<span class="k">${k}</span><span class="v">${v}</span>`).join('');
  const building = st == null ? Math.floor((100 * (state.building?.done ?? 0)) / Math.max(1, state.building?.total ?? 1)) : null;
  measureEl.innerHTML =
    `<h2>Measure<span class="spacer"></span><button class="ibtn" data-clear data-tip="Clear cursors (Esc)" aria-label="Clear cursors"><svg viewBox="0 0 16 16"><path d="M4 4l8 8M12 4l-8 8"/></svg></button></h2>` +
    `<div class="mgrid">${rows([
      ['A', readoutTime(state.A, span)],
      ['B', readoutTime(state.B, span)],
      ['Δt', duration(dt)],
      ['1/Δt', frequency(1e9 / Math.abs(dt))],
    ])}</div>` +
    chans
      .map((ch) => {
        const c = f.info.channels[ch];
        const [ya, yb] = [at(va, ch), at(vb, ch)];
        const s = st?.find((x) => x.channel === ch);
        const stats = s
          ? rows([
              ['Min', value(s.min)],
              ['Max', value(s.max)],
              ['Mean', value(s.mean)],
              ['Std dev', value(s.std)],
              ['Samples', s.count.toLocaleString()],
            ])
          : '';
        return `<div class="mchan"><span class="chip" style="--c:${colorOf(ch)}">${esc(c.name)}${c.unit ? `<span class="unit">${esc(c.unit)}</span>` : ''}</span><div class="mgrid">${rows([
          ['At A', value(ya)],
          ['At B', value(yb)],
          ['Δy', ya != null && yb != null ? value(yb - ya) : '—'],
        ])}${stats}</div></div>`;
      })
      .join('') +
    (building != null ? `<p class="mnote">Reading ${esc(f.info.path.split(/[\\/]/).pop() ?? '')}… exact stats between the cursors once it's read (${building}%).</p>` : '');
  applyTips();
}

let resizeScheduled = false;
function scheduleResize(): void {
  if (resizeScheduled) return;
  resizeScheduled = true;
  requestAnimationFrame(() => {
    resizeScheduled = false;
    scheduleDraw();
    overviewChanged();
  });
}

// ---------- opening ----------

async function openPath(path: string): Promise<void> {
  ui.note = '';
  ui.undo = null;
  try {
    const progress = new Channel<Progress>();
    progress.onmessage = onProgress;
    const o = await invoke<Opened>('open', { path, progress });
    if (!o.range) {
      void invoke('close', { file: o.id });
      throw new Error('it has no samples');
    }
    // the file open so far stays until the new one is open
    if (state.file) void invoke('close', { file: state.file.id });
    state.file = { id: o.id, info: o.info, first: BigInt(o.range[0]), last: BigInt(o.range[1]) };
    state.building = o.status.state === 'built' ? null : { done: 0, total: o.info.samples, until: null };
    const saved = settings.layouts[o.info.path];
    const n = o.info.channels.length;
    stack.panes = saved?.every((p) => p.channels?.every((ch) => ch < n)) ? saved.map((p) => ({ ...p, channels: [...p.channels] })) : defaultPanes(n);
    stack.sel = 0;
    normalize();
    state.layers.forEach((l) => l?.traces.forEach((t) => renderer.free(t.gl)));
    state.ov?.traces.forEach((t) => renderer.free(t.gl));
    state.layers = [];
    state.coarseSince = [];
    state.hatchFrom = null;
    state.ov = null;
    state.ovStale = true;
    state.A = state.B = null;
    state.history = [];
    state.future = [];
    state.exactMs = Infinity;
    ui.file = path.split(/[\\/]/).pop() ?? path;
    ui.path = path;
    ui.measureDismissed = false;
    ui.channels = o.info.channels.map((c) => ({ name: c.name, unit: c.unit }));
    $('empty').hidden = true;
    await tick();
    renderPanes();
    setView(state.file.first, state.file.last + 1n);
    void refreshMeasure();
  } catch (e) {
    note(`Couldn't open ${path.split(/[\\/]/).pop()}: ${e}`);
  }
}

let progressRefresh = 0;
function onProgress(p: Progress): void {
  if (!state.file) return;
  if (p.error) {
    note(`Couldn't read ${ui.file} to the end: ${p.error}`);
    return;
  }
  const done = p.done >= p.total;
  state.building = done ? null : { done: p.done, total: p.total, until: p.builtUntil != null ? BigInt(p.builtUntil) : null };
  if (!done) ui.perf = `Reading ${Math.floor((100 * p.done) / Math.max(1, p.total))}%`;
  // new summaries: fill in the panes (at most four times a second)
  const now = performance.now();
  if (done || now - progressRefresh > 250) {
    progressRefresh = now;
    // what was just read is in memory: exact views of it come quickly, with no coarse one first
    state.ovStale = true;
    void requestView('exact');
    if (done && state.A != null && state.B != null) void refreshMeasure();
  }
  scheduleDraw();
}

/** Explorer's Open folder: the native picker, then the folder's recordings listed (it is only read). */
async function chooseFolder(): Promise<void> {
  const path = await openDialog({ directory: true, multiple: false });
  if (typeof path === 'string') await showFolder(path);
}

async function showFolder(path: string): Promise<void> {
  try {
    ui.folder = await invoke<FolderEntry>('list_folder', { path });
    if (settings.folder !== path) {
      setSetting('folder', path);
      setSetting('exExpanded', ui.folder.children.filter((c) => c.dir).map((c) => c.path));
    }
  } catch (e) {
    ui.folder = null;
    setSetting('folder', '');
    note(`Couldn't list ${path}: ${e}`);
  }
}

/** Closes the open file: the panes empty, the folder stays listed. */
function closeFile(): void {
  const f = state.file;
  if (!f) return;
  void invoke('close', { file: f.id });
  state.layers.forEach((l) => l?.traces.forEach((t) => renderer.free(t.gl)));
  state.ov?.traces.forEach((t) => renderer.free(t.gl));
  Object.assign(state, { file: null, layers: [], ov: null, building: null, A: null, B: null, hoverT: null, history: [], future: [] });
  state.hover.clear();
  stack.panes = [];
  stack.values = {};
  Object.assign(ui, { file: '', path: '', channels: [], range: '', span: '', pointer: '', canBack: false, cursors: false, anyCursor: false, perf: '' });
  $('empty').hidden = false;
  renderPanes();
  renderOverviewHead();
  void refreshMeasure();
}

async function chooseFile(): Promise<void> {
  const path = await openDialog({ multiple: false, filters: [{ name: 'Recordings', extensions: ['csv', 'tsv', 'tlm'] }] });
  if (typeof path === 'string') await openPath(path);
}

// ---------- interaction ----------

const setMode = (m: Mode) => setSetting('mode', m);

/** The pane and plot rectangle under a pointer event, if any. */
function paneAt(e: { clientX: number; clientY: number }): { i: number; r: Rect; x: number } | null {
  const c = center.getBoundingClientRect();
  const x = e.clientX - c.left;
  const y = e.clientY - c.top;
  for (let i = 0; i < stack.panes.length; i++) {
    const r = plotRect(i);
    const inX = x >= r.x - PAD.left && x <= r.x + r.w + PAD.right;
    if (inX && y >= r.y - 4 && y <= r.y + r.h + 4) return { i, r, x };
  }
  return null;
}

function nearCursor(r: Rect, x: number): 'A' | 'B' | null {
  let best: 'A' | 'B' | null = null;
  let bd = 7;
  for (const k of ['A', 'B'] as const) {
    const t = state[k];
    if (t == null || t < state.t0 || t > state.t1) continue;
    const d = Math.abs(xAt(r, t) - x);
    if (d < bd) [bd, best] = [d, k];
  }
  return best;
}

function placeCursor(which: 'A' | 'B', t: bigint): void {
  state[which] = t;
  scheduleDraw();
  void refreshMeasure();
}

type Drag =
  | { type: 'pan'; cx: number; t0: bigint; t1: bigint; w: number }
  | { type: 'box'; pane: number; r: Rect }
  | { type: 'cursor'; which: 'A' | 'B'; r: Rect }
  | { type: 'overview'; r: Rect; offset: bigint };
let drag: Drag | null = null;
let lastPointer: { clientX: number; clientY: number } | null = null;

stackEl.addEventListener('pointerdown', (e) => {
  if (!state.file) return;
  const pane = (e.target as HTMLElement).closest<HTMLElement>('[data-pane]');
  if (pane) selectPane(Number(pane.dataset.pane));
  if (!(e.target as HTMLElement).closest('.plot')) return;
  const hit = paneAt(e);
  if (!hit) return;
  const panGesture = e.button === 1 || (e.button === 0 && e.shiftKey);
  if (e.button !== 0 && !panGesture) return;
  e.preventDefault();
  stopGlide();
  stackEl.setPointerCapture(e.pointerId);
  const near = nearCursor(hit.r, hit.x);
  if (near && !panGesture) {
    drag = { type: 'cursor', which: near, r: hit.r };
  } else if (settings.mode === 'cursor' && !panGesture) {
    const t = tAt(hit.r, hit.x);
    const which = state.A == null ? 'A' : state.B == null ? 'B' : abs(t - state.A) < abs(t - state.B) ? 'A' : 'B';
    drag = { type: 'cursor', which, r: hit.r };
    placeCursor(which, t);
  } else if (settings.mode === 'box' && !panGesture) {
    drag = { type: 'box', pane: hit.i, r: hit.r };
    state.box = { pane: hit.i, x0: hit.x, x1: hit.x };
  } else {
    drag = { type: 'pan', cx: e.clientX, t0: state.t0, t1: state.t1, w: hit.r.w };
    center.classList.add('dragging');
  }
});

const abs = (v: bigint) => (v < 0n ? -v : v);

stackEl.addEventListener('pointermove', (e) => {
  lastPointer = e;
  const hit = paneAt(e);
  if (drag?.type === 'pan') {
    const shift = BigInt(Math.round(((drag.cx - e.clientX) / drag.w) * Number(drag.t1 - drag.t0)));
    setView(drag.t0 + shift, drag.t1 + shift);
  } else if (drag?.type === 'box' && state.box) {
    const x = e.clientX - center.getBoundingClientRect().left;
    state.box.x1 = Math.max(drag.r.x, Math.min(drag.r.x + drag.r.w, x));
    scheduleDraw();
  } else if (drag?.type === 'cursor') {
    const x = e.clientX - center.getBoundingClientRect().left;
    placeCursor(drag.which, tAt(drag.r, Math.max(drag.r.x, Math.min(drag.r.x + drag.r.w, x))));
  }
  const over = hit && hit.x >= hit.r.x && hit.x <= hit.r.x + hit.r.w;
  state.hoverT = over ? tAt(hit.r, hit.x) : null;
  void requestHover(state.hoverT);
  updateChips();
  scheduleDraw();
});

stackEl.addEventListener('pointerleave', () => {
  if (drag) return;
  lastPointer = null;
  state.hoverT = null;
  void requestHover(null);
  scheduleDraw();
});

const endDrag = () => {
  if (drag?.type === 'box' && state.box) {
    const [a, b] = [Math.min(state.box.x0, state.box.x1), Math.max(state.box.x0, state.box.x1)];
    if (b - a >= 4) glideTo(tAt(drag.r, a), tAt(drag.r, b));
    state.box = null;
    scheduleDraw();
  }
  drag = null;
  center.classList.remove('dragging');
};
stackEl.addEventListener('pointerup', endDrag);
stackEl.addEventListener('pointercancel', endDrag);
stackEl.addEventListener('dblclick', (e) => {
  if (settings.dblFit && (e.target as HTMLElement).closest('.plot')) fit();
});

stackEl.addEventListener(
  'wheel',
  (e) => {
    const hit = paneAt(e);
    if (!state.file || !hit) return;
    e.preventDefault();
    stopGlide();
    const span = Number(state.t1 - state.t0);
    let dy = e.deltaY;
    if (e.deltaMode === 1) dy *= 16;
    if (Math.abs(e.deltaX) > Math.abs(dy)) {
      const shift = BigInt(Math.round((e.deltaX / hit.r.w) * span));
      setView(state.t0 + shift, state.t1 + shift);
      return;
    }
    const speed = { slow: 0.5, normal: 1, fast: 2 }[settings.zoomSpeed];
    const factor = Math.exp(Math.max(-200, Math.min(200, dy)) * (e.ctrlKey ? 0.01 : 0.0015) * speed);
    const frac = Math.min(1, Math.max(0, (hit.x - hit.r.x) / hit.r.w));
    const anchor = frac * span;
    const t0 = state.t0 + BigInt(Math.round(anchor - anchor * factor));
    setView(t0, t0 + BigInt(Math.max(1, Math.round(span * factor))));
  },
  { passive: false },
);

// overview: drag the window, or click elsewhere to move it there
center.addEventListener('pointerdown', (e) => {
  const f = state.file;
  const target = e.target as HTMLElement;
  if (target.closest('[data-ovtop]')) {
    setSetting('overviewTop', !settings.overviewTop);
    return;
  }
  if (!f || !target.closest('.ovplot') || e.button !== 0) return;
  const r = ovRect()!;
  const c = center.getBoundingClientRect();
  const span = f.last + 1n - f.first;
  const t = f.first + BigInt(Math.round(((e.clientX - c.left - r.x) / r.w) * Number(span)));
  stopGlide();
  if (t < state.t0 || t > state.t1) {
    const half = (state.t1 - state.t0) / 2n;
    setView(t - half, t + half, true);
  }
  drag = { type: 'overview', r, offset: t - state.t0 };
  center.setPointerCapture(e.pointerId);
});
center.addEventListener('pointermove', (e) => {
  const f = state.file;
  if (drag?.type !== 'overview' || !f) return;
  const c = center.getBoundingClientRect();
  const t = f.first + BigInt(Math.round(((e.clientX - c.left - drag.r.x) / drag.r.w) * Number(f.last + 1n - f.first)));
  const span = state.t1 - state.t0;
  setView(t - drag.offset, t - drag.offset + span);
});
center.addEventListener('pointerup', () => {
  if (drag?.type === 'overview') drag = null;
});

$('addPane').addEventListener('click', () => addPaneBelow(stack.panes.length - 1));
onStackChange(() => void layoutChanged());

measureEl.addEventListener('click', (e) => {
  if ((e.target as HTMLElement).closest('[data-clear]')) clearCursors();
});

// ---------- channel menu and gap factor ----------

let popup: HTMLElement | null = null;
function closePopup(): boolean {
  if (!popup) return false;
  popup.remove();
  popup = null;
  return true;
}

/** Places a popup at (x, y), kept inside the window. */
function place(el: HTMLElement, x: number, y: number): void {
  closePopup();
  document.body.append(el);
  popup = el;
  el.style.left = `${Math.max(8, Math.min(window.innerWidth - el.offsetWidth - 8, x))}px`;
  el.style.top = `${Math.max(8, Math.min(window.innerHeight - el.offsetHeight - 8, y))}px`;
}

function channelMenu(ch: number, x: number, y: number): void {
  const f = state.file;
  if (!f) return;
  const el = document.createElement('div');
  el.className = 'menu';
  el.setAttribute('role', 'menu');
  el.innerHTML = `<div class="mgrp">${esc(f.info.channels[ch].name)}</div><button class="mi" role="menuitem" data-gap><span class="nm">Gap factor…</span><span class="hint num">${gapFactor(ch)}×</span></button>`;
  el.querySelector('[data-gap]')!.addEventListener('click', () => gapDialog(ch, x, y));
  place(el, x, y);
}

/** Sets how far apart samples of channel `ch` must be to break its line, in median intervals. */
function gapDialog(ch: number, x: number, y: number): void {
  const f = state.file;
  if (!f) return;
  const c = f.info.channels[ch];
  const el = document.createElement('div');
  el.className = 'dlg';
  el.setAttribute('role', 'dialog');
  el.setAttribute('aria-label', `Gap factor of ${c.name}`);
  const note =
    f.info.time.kind === 'column'
      ? ''
      : '<p class="dnote">This file has a fixed sample rate, so its lines have no gaps to break.</p>';
  el.innerHTML =
    `<header><b>Gap factor</b><span class="dsub">${esc(c.name)}</span></header>` +
    `<div class="dbody"><p>Break the line where samples are more than this many times the usual interval apart.</p>` +
    `<label class="dfield"><input class="num" type="number" min="${GAP_MIN}" step="0.5" value="${gapFactor(ch)}"><span>× the median interval</span></label>` +
    `<p class="dnote" data-err hidden>Use ${GAP_MIN} or more.</p>${note}</div>` +
    `<footer><button class="linkbtn" data-reset>Reset to ${GAP_DEFAULT}</button><span class="spacer"></span><button class="btn" data-cancel>Cancel</button><button class="btn primary" data-apply>Apply</button></footer>`;
  const input = el.querySelector('input')!;
  const err = el.querySelector<HTMLElement>('[data-err]')!;
  const apply = (v: number) => {
    if (!(v >= GAP_MIN)) {
      err.hidden = false;
      input.focus();
      return;
    }
    const byCh = (settings.gapFactors[f.info.path] ??= {});
    if (v === GAP_DEFAULT) delete byCh[ch];
    else byCh[ch] = v;
    if (!Object.keys(byCh).length) delete settings.gapFactors[f.info.path];
    saveSettings();
    closePopup();
    // the gap index keeps every jump over 2× the median, so this only redraws
    if (panes()[stack.sel]?.includes(ch)) overviewChanged();
    else void requestView();
  };
  el.querySelector('[data-apply]')!.addEventListener('click', () => apply(input.valueAsNumber));
  el.querySelector('[data-reset]')!.addEventListener('click', () => apply(GAP_DEFAULT));
  el.querySelector('[data-cancel]')!.addEventListener('click', () => closePopup());
  input.addEventListener('input', () => (err.hidden = true));
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') apply(input.valueAsNumber);
  });
  place(el, x, y);
  input.focus();
  input.select();
}

function onChannelContext(e: MouseEvent): void {
  const hit = (e.target as HTMLElement).closest<HTMLElement>('[data-ch]');
  if (!hit) return;
  e.preventDefault();
  channelMenu(Number(hit.dataset.ch), e.clientX, e.clientY);
}
$('side').addEventListener('contextmenu', onChannelContext);
stackEl.addEventListener('contextmenu', onChannelContext);
document.addEventListener('pointerdown', (e) => {
  if (popup && !(e.target as HTMLElement).closest('.menu, .dlg')) closePopup();
});

function clearCursors(): void {
  state.A = state.B = null;
  ui.measureDismissed = false;
  scheduleDraw();
  void refreshMeasure();
}

window.addEventListener('keydown', (e) => {
  const k = e.key.toLowerCase();
  if ((e.metaKey || e.ctrlKey) && k === 'o') {
    e.preventDefault();
    void chooseFile();
    return;
  }
  if (e.key === 'Escape' && closePopup()) return;
  if (popup) return;
  if (e.metaKey || e.ctrlKey) return;
  if (e.altKey && e.key === 'ArrowLeft') return back();
  if (e.altKey && e.key === 'ArrowRight') return forward();
  if (e.key === 'Escape') return clearCursors();
  if (!state.file) return;
  // A and B over a plot place the cursors there; elsewhere B is Box zoom
  const hit = lastPointer && paneAt(lastPointer);
  if ((k === 'a' || k === 'b') && hit && state.hoverT != null) return placeCursor(k === 'a' ? 'A' : 'B', state.hoverT);
  if (k === 'b') setMode('box');
  else if (k === 'p') setMode('pan');
  else if (k === 'c') setMode('cursor');
  else if (k === 'f') fit();
});

Object.assign(actions, {
  open: () => void chooseFile(),
  openFolder: () => void chooseFolder(),
  openPath: (p: string) => void (p !== state.file?.info.path && openPath(p)),
  closeFile,
  fit,
  back,
  clearCursors,
});
onSetting((key) => {
  if (key === 'theme') {
    applyTheme();
    tokenCache = null;
    renderPanes();
  } else if (key === 'overviewTop') {
    renderOverviewHead();
    scheduleResize();
  }
});
new ResizeObserver(scheduleResize).observe(center);

void getCurrentWebview().onDragDropEvent((e) => {
  const p = e.payload;
  center.classList.toggle('drop', p.type === 'over' || p.type === 'enter');
  if (p.type === 'drop' && p.paths.length) void openPath(p.paths[0]);
});

// tooltips: `data-tip` (CLAUDE.md); the native title for now
function applyTips(): void {
  for (const el of document.querySelectorAll<HTMLElement>('[data-tip]')) el.title = el.dataset.tip ?? '';
}
applyTips();
applyTheme();
updateRange();
void refreshMeasure();

/** What the gate scripts drive. */
export const gateApi = {
  get file() {
    return state.file;
  },
  get panes() {
    return panes();
  },
  widthPx,
  /** Sets the view without scheduling a frame: the gate draws in its own animation frames. */
  setView(t0: bigint, t1: bigint) {
    state.t0 = t0;
    state.t1 = t1;
    void requestView();
  },
  draw,
  open: openPath,
  set onQuery(f: ((q: QueryStats) => void) | null) {
    state.onQuery = f;
  },
  set onPaint(f: ((p: { coarse: boolean; complete: boolean }) => void) | null) {
    state.onPaint = f;
  },
  /** Resolves when the pyramid is built. */
  built(): Promise<void> {
    return new Promise((done) => {
      const check = () => (state.building ? setTimeout(check, 50) : done());
      check();
    });
  },
};

/** What the UI tests read (app/ui/tests): the view, cursors and build state, as plain values. */
(window as unknown as { __tlmApp: unknown }).__tlmApp = {
  view: () => ({ t0: String(state.t0), t1: String(state.t1), back: state.history.length }),
  cursors: () => ({ A: state.A == null ? null : String(state.A), B: state.B == null ? null : String(state.B) }),
  hoverT: () => (state.hoverT == null ? null : String(state.hoverT)),
  building: () => state.building && { done: state.building.done, total: state.building.total },
  panes: () => panes().map((p) => [...p]),
  layout: () => stack.panes.map((p) => (p.beside ? 'beside' : p.under ? 'under' : 'row')),
  sel: () => stack.sel,
  mode: () => settings.mode,
  drawn: () => state.layers.map((l) => (l ? [...l.traces.keys()] : null)),
};

if (settings.folder) void showFolder(settings.folder);
const launch = await invoke<{ file: string | null; gate: '1' | '2' | null; gateMode: string | null }>('launch');
if (launch.gate === '2' && launch.file) await runGate2(gateApi, launch.file, launch.gateMode === 'first' ? 'first' : 'reopen');
else {
  if (launch.file) await openPath(launch.file);
  if (launch.gate === '1') await runGate(gateApi);
}
