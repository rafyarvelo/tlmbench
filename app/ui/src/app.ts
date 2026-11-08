// Phase 2 UI: a stack of panes on one time axis, an overview of the whole recording, hover readouts and cursors
// with Measure. Pan and zoom re-project the last reply at once (two uniforms per trace) while the core computes
// the sharper one; replies carry a generation and older ones are dropped. At most one query is in flight, and
// the latest view is asked for as soon as it returns. One query asks for every pane (each at its own width) and,
// when it needs redrawing, the overview, so the stack redraws together.
//
// On a cold file the first reply is coarse (from whole summaries, labelled), and the exact one follows. While the
// pyramid is built, panes fill in left to right and the part not read yet is hatched.

import { Channel, invoke } from '@tauri-apps/api/core';
import { mount, tick, unmount } from 'svelte';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

import { type AxesStyle, drawAxes, duration, fileTime, frequency, PAD, readoutTime, setTimeFormat, spanText, timeLabel, type YAxis } from './axes';
import { type LiveHealth, runGate, runGate2, runGate3 } from './gate';
import { type Axis, type GlTrace, type Rect, Renderer, type Rgba } from './gl';
import { groupChannels, overrideKey } from './groups';
import Menu, { type MenuItem } from './shell/Menu.svelte';
import { bindTips, flash } from './tips';
import { applyTheme, type StarterPanes, shownTheme, type Mode, onSetting, saveSettings, type Session, setSetting, settings } from './settings.svelte';
import { type Health, live, liveActions } from './live.svelte';
import { play, playActions, SPEEDS } from './play.svelte';
import {
  addPaneBelow,
  assignColors,
  colorIdx,
  duplicate,
  fmtCh,
  learnDecimals,
  type AxisKey,
  axisKey,
  axisOf,
  type AxisSettings,
  hasChart,
  labelled,
  normalize,
  note,
  onStackChange,
  type Pane,
  rightPad,
  setAxis,
  stack,
  units,
} from './stack.svelte';
import { tabs } from './tabs.svelte';
import { actions, type FolderEntry, ui } from './ui.svelte';
import { type Batch, decodeBatch, type Reply } from './wire';
import { fromWorkspace, isWorkspace, toWorkspace, viewFile, type Workspace } from './workspaces';

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
  /** Each channel's trace; `y` holds log10 values when it was uploaded for a log axis (`log`). */
  traces: Map<number, { gl: GlTrace; x: Float32Array; y: Float32Array; log: boolean; yMin: number; yMax: number; coarse: boolean; points: number; raw: boolean }>;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const center = $<HTMLDivElement>('center');
const stackEl = $<HTMLDivElement>('stack');
const overlay = $<HTMLCanvasElement>('overlay');
const renderer = new Renderer($<HTMLCanvasElement>('gl'));

// ---------- state ----------

/** An open file: its id in the core, what it holds, and its first and last sample times. */
type OpenFile = { id: number; info: RecordingInfo; first: bigint; last: bigint };

const state = {
  file: null as null | OpenFile,
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
  /** A box being dragged on the overview (x from the left of the centre area). */
  ovBox: null as null | { x0: number; x1: number },
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
const colorOf = (ch: number) => tokens()[`--trace-${colorIdx(ch) + 1}`];
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
  return rectIn(el, { l: PAD.left, r: rightPad(), t: 4, b: labelled(i) ? PAD.timeLabels : 4 });
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
const hasPlot = hasChart;

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

/** Puts a reply's traces on the GPU; channels `logOf` names are uploaded as log10 values (non-positive ones as gaps). */
function upload(layer: Layer | null, reply: Reply, logOf: (ch: number) => boolean = () => false): Layer {
  const next: Layer = { originNs: reply.originNs, xScaleNs: reply.xScaleNs, builtUntil: reply.builtUntil, traces: new Map() };
  for (const t of reply.traces) {
    const old = layer?.traces.get(t.channel);
    layer?.traces.delete(t.channel);
    const log = logOf(t.channel);
    let [y, yMin, yMax] = [t.y, t.yMin, t.yMax];
    if (log) {
      y = t.y.map((v) => (v > 0 ? Math.log10(v) : NaN));
      yMax = t.yMax > 0 ? Math.log10(t.yMax) : NaN;
      yMin = t.yMin > 0 ? Math.log10(t.yMin) : y.reduce((a, v) => (v < a ? v : a), Infinity);
    }
    next.traces.set(t.channel, {
      gl: renderer.upload(t.x, y, old?.gl, t.dashed),
      x: t.x,
      y,
      log,
      yMin,
      yMax,
      coarse: t.coarse,
      points: t.x.length,
      raw: t.kind === 'raw',
    });
  }
  layer?.traces.forEach((t) => renderer.free(t.gl));
  return next;
}

/** A pane's or the overview's request: its channels with their gap factors, over its own width. */
/** One pane's (or the overview's) part of a view request; `rawMax`: Draw every sample up to this many in view. */
function entry(id: number, channels: number[], t0: bigint, t1: bigint, width: number, rawMax = 0) {
  return { id, channels, gapFactors: channels.map(gapFactor), t0: t0.toString(), t1: t1.toString(), widthPx: width, rawMax, connectGaps: settings.gapLines === 'connect' };
}

const LIVE_VIEW_EVERY_MS = 33;
let lastViewAt = 0;
let viewTimer = 0;

/** Asks for every pane and, when stale, the overview, in one query under one generation. */
async function requestView(precision?: 'coarse' | 'exact'): Promise<void> {
  const f = state.file;
  if (!f || !stack.panes.length) return;
  if (state.inflight) {
    state.pending = true;
    return;
  }
  // following a live source, views are asked for at most 30 times a second (the design's display rate); frames in
  // between re-project the last reply
  const wait = live.on && live.following ? LIVE_VIEW_EVERY_MS - (performance.now() - lastViewAt) : 0;
  if (wait > 0) {
    if (!viewTimer) viewTimer = window.setTimeout(() => ((viewTimer = 0), void requestView(precision)), wait);
    return;
  }
  lastViewAt = performance.now();
  state.inflight = true;
  state.pending = false;
  const generation = ++state.generation;
  const want = precision ?? (state.exactMs > 150 ? 'coarse' : 'exact');
  const inView = samplesInView();
  // values and table panes have no plot to ask for
  const rawMax = Number(settings.drawRaw) || 0;
  const entries = panes().flatMap((chans, i) => (hasPlot(i) ? [entry(i, chans, state.t0, state.t1, widthPx(i), rawMax)] : []));
  const r = ovRect();
  const withOv = state.ovStale && !!r;
  if (withOv && r) {
    const w = Math.max(1, Math.round(r.w * (window.devicePixelRatio || 1)));
    entries.push(entry(OVERVIEW, ovChans(), f.first, f.last + 1n, w));
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
        return reply ? upload(state.layers[i] ?? null, reply, (ch) => !!axisOf(i, axisKey(i, ch)).log) : (state.layers[i] ?? null);
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
    void requestResting();
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

// value tiles off the charts: cursor A, else the end of the view
let restingWanted = 0;
async function requestResting(): Promise<void> {
  const f = state.file;
  const chans = [...new Set(stack.panes.filter((p) => p.mode === 'values').flatMap((p) => p.channels))];
  const mine = ++restingWanted;
  setValuesAt();
  if (!f || !chans.length) {
    if (Object.keys(stack.resting).length) stack.resting = {};
    return;
  }
  const t = state.A ?? (state.t1 > f.last ? f.last : state.t1);
  const got = await invoke<Nearest[]>('values_at', { file: f.id, channels: chans, t: t.toString() });
  if (mine !== restingWanted) return;
  stack.resting = Object.fromEntries(got.map((n) => [n.channel, fmtCh(f.id, n.channel, n.value)]));
}

// ---------- view ----------

/** True (with a note and Unlock) when the view is locked: gestures call this before changing it. */
function locked(): boolean {
  if (!settings.locked) return false;
  note('The view is locked.', () => setSetting('locked', false), 'Unlock');
  return true;
}

function setView(t0: bigint, t1: bigint, remember = false): void {
  const f = state.file;
  if (!f) return;
  scheduleSessionSave();
  if (remember) {
    state.history.push([state.t0, state.t1]);
    state.future = [];
  }
  // the view stays inside the recording, as in the prototype: at most all of it, and a pan stops at either end
  const [lo, hi] = [f.first, f.last + 1n];
  let span = t1 - t0;
  const minSpan = 1000n; // 1 µs
  const maxSpan = hi - lo;
  if (span < minSpan || span > maxSpan) {
    if (span < minSpan && lastPointer) flash('Maximum zoom', lastPointer.clientX, lastPointer.clientY);
    const mid = t0 + span / 2n;
    span = span < minSpan ? minSpan : maxSpan;
    t0 = mid - span / 2n;
  }
  if (t0 < lo) t0 = lo;
  if (t0 + span > hi) t0 = hi - span;
  state.t0 = t0;
  state.t1 = t0 + span;
  // any view you choose stops following the playhead (it keeps going); Follow brings it back
  if (play.open && play.follow && !playDriving) play.follow = false;
  // and in a live tab, stops following now; Jump to live comes back
  if (live.on && live.following && !liveDriving) live.following = false;
  if (live.on) live.back = Math.max(0, Number(f.last - state.t1) / 1e9);
  updateRange();
  scheduleDraw();
  void requestView();
  if (state.hoverT == null) void requestResting();
  if (state.A == null || state.B == null) scheduleMeasure();
}

// ---------- Play: a playhead moves through the file and the view follows it ----------

let playDriving = false;
let playRaf = 0;
let playLast = 0;
/** The playhead as a time, and a time as seconds from the file's first sample. */
const playNs = () => (state.file ? state.file.first + BigInt(Math.round(play.t * 1e9)) : 0n);
const secsOf = (t: bigint) => (state.file ? Number(t - state.file.first) / 1e9 : 0);

/** Opens the transport: the playhead at cursor A, or near the end of the view, at a speed that plays the file in
 * about 3 minutes, with the window the view has (or a 20th of the file). */
function openPlay(): void {
  const f = state.file;
  if (!f || play.open) return;
  const end = Number(f.last - f.first) / 1e9;
  const span = Number(state.t1 - state.t0) / 1e9;
  Object.assign(play, {
    open: true,
    running: false,
    end,
    t: Math.max(0, Math.min(end, state.A != null ? secsOf(state.A) : secsOf(state.t0) + span * 0.85)),
    speed: Math.max(1, SPEEDS.find((s) => end / s <= 180) ?? SPEEDS[SPEEDS.length - 1]),
    win: Math.max(4e-6, span < end * 0.9 ? span : end / 20),
    follow: true,
    loop: false,
    canLoop: state.A != null && state.B != null,
  });
  followPlayhead();
}

function closePlay(): void {
  Object.assign(play, { open: false, running: false });
  cancelAnimationFrame(playRaf);
  playRaf = 0;
  scheduleDraw();
}

/** The A–B span when looping, in seconds. */
function loopSpan(): [number, number] | null {
  if (!play.loop || state.A == null || state.B == null || state.A === state.B) return null;
  const [a, b] = [secsOf(state.A), secsOf(state.B)];
  return [Math.min(a, b), Math.max(a, b)];
}

function togglePlay(): void {
  if (!play.open) openPlay();
  if (!play.open) return;
  play.running = !play.running;
  const lp = loopSpan();
  if (play.running && lp && (play.t < lp[0] || play.t >= lp[1])) play.t = lp[0];
  if (play.running && play.t >= play.end - 1e-9) play.t = 0;
  if (play.running) {
    play.follow = true;
    playLast = performance.now();
    if (!playRaf) playRaf = requestAnimationFrame(playFrame);
    followPlayhead();
  }
}

/** The view, following: the playhead at 85% of the window, so a little of what comes shows. */
function followPlayhead(): void {
  const f = state.file;
  if (!f || !play.open || !play.follow) return;
  let t1 = play.t + play.win * 0.15;
  let t0 = t1 - play.win;
  if (t0 < 0) [t0, t1] = [0, t1 - t0];
  if (t1 > play.end) [t0, t1] = [Math.max(0, t0 - (t1 - play.end)), play.end];
  playDriving = true;
  setView(f.first + BigInt(Math.round(t0 * 1e9)), f.first + BigInt(Math.round(t1 * 1e9)) + 1n);
  playDriving = false;
}

function playFrame(now: number): void {
  playRaf = 0;
  if (!play.open || !play.running) return;
  const dt = Math.min(0.25, (now - playLast) / 1000);
  playLast = now;
  const before = play.t;
  let t = play.t + dt * play.speed;
  const lp = loopSpan();
  if (lp && before < lp[1] && t >= lp[1]) t = lp[0];
  else if (t >= play.end) [t, play.running] = [play.end, false];
  play.t = t;
  followPlayhead();
  // readouts follow the playhead unless the pointer is on a chart
  if (state.hoverT == null) void requestHover(playNs());
  scheduleDraw();
  if (play.running) playRaf = requestAnimationFrame(playFrame);
}

function seekPlay(t: number): void {
  play.t = Math.max(0, Math.min(play.end, t));
  play.follow = true;
  followPlayhead();
  if (state.hoverT == null) void requestHover(playNs());
  scheduleDraw();
}

Object.assign(playActions, {
  open: openPlay,
  toggle: togglePlay,
  seek: seekPlay,
  setWindow: (w: number) => {
    play.win = Math.min(w, play.end || w);
    play.follow = true;
    followPlayhead();
  },
  follow: () => {
    play.follow = true;
    followPlayhead();
  },
  close: closePlay,
});

/** The playhead across a plot. */
function drawPlayhead(ctx: CanvasRenderingContext2D, r: Rect, tk: Record<string, string>, t0 = state.t0, t1 = state.t1): void {
  if (!play.open || !state.file) return;
  const t = playNs();
  if (t < t0 || t > t1) return;
  const x = Math.round(r.x + (Number(t - t0) / Number(t1 - t0)) * r.w) + 0.5;
  ctx.save();
  ctx.strokeStyle = tk['--accent'];
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.moveTo(x, r.y);
  ctx.lineTo(x, r.y + r.h);
  ctx.stroke();
  ctx.restore();
}

/** Glides to a view over about 0.3 s on its centre and log(span), so a 2× and a 200× zoom look even. */
let glide = 0;
function glideTo(t0: bigint, t1: bigint): void {
  if (locked()) return;
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
  if (state.history.length && locked()) return;
  const v = state.history.pop();
  if (!v) return;
  state.future.push([state.t0, state.t1]);
  stopGlide();
  setView(v[0], v[1]);
}

function forward(): void {
  if (state.future.length && locked()) return;
  const v = state.future.pop();
  if (!v) return;
  state.history.push([state.t0, state.t1]);
  stopGlide();
  setView(v[0], v[1]);
}

function updateRange(): void {
  const span = Number(state.t1 - state.t0);
  ui.range = state.file ? `${ui.time === 'samples' ? 'Sample ' : ''}${readoutTime(state.t0, span)} → ${readoutTime(state.t1, span)}` : '';
  ui.span = state.file ? `${spanText(span)} visible` : '';
  ui.canBack = state.history.length > 0;
}

/** What table panes read: the file, the time view and the cursors (only written when they change). */
function syncView(): void {
  const v = ui.view;
  const file = state.file?.id ?? null;
  const [t0, t1] = [state.t0.toString(), state.t1.toString()];
  const [A, B] = [state.A?.toString() ?? null, state.B?.toString() ?? null];
  const [first, end] = [state.file?.first.toString() ?? '0', state.file ? (state.file.last + 1n).toString() : '1'];
  if (v.file !== file || v.t0 !== t0 || v.t1 !== t1 || v.first !== first || v.end !== end || v.A !== A || v.B !== B) ui.view = { file, t0, t1, first, end, A, B };
  if (v.file !== file) assignColors();
}

// ---------- drawing ----------

function draw(): void {
  state.drawScheduled = false;
  syncView();
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

  // panes scroll inside the stack: nothing is drawn outside its visible part
  const clip = rectIn(stackEl, { l: 0, r: 0, t: 0, b: 0 });
  panes().forEach((chans, i) => {
    if (!hasPlot(i)) return;
    const r = plotRect(i);
    if (r.y + r.h + PAD.timeLabels < clip.y || r.y - 4 > clip.y + clip.h) return;
    ctx.save();
    ctx.beginPath();
    ctx.rect(clip.x, clip.y, clip.w, clip.h);
    ctx.clip();
    const main = state.layers[i];
    // each axis's range: what its traces hold in view, or what its settings say
    const keys: AxisKey[] = units(i).length > 1 ? ['u0', 'u1'] : ['u0'];
    const keyOf = new Map(chans.map((ch) => [ch, keys.length > 1 ? axisKey(i, ch) : 'u0']));
    const ranges = {} as Record<AxisKey, YAxis>;
    for (const key of keys) {
      const ax = axisOf(i, key);
      const ts = [];
      for (const ch of chans) {
        if (keyOf.get(ch) !== key) continue;
        const t = main?.traces.get(ch);
        // a trace uploaded before its axis turned log (or back) waits for the next reply
        if (!t || t.log !== !!ax.log) {
          complete = false;
          continue;
        }
        coarse ||= t.coarse;
        ts.push(t);
      }
      ranges[key] = { ...axisRange(ax, ts), log: ax.log, dec: ax.dec, label: ax.label };
    }
    paneY[i] = ranges;
    if (state.file)
      for (const ch of chans) {
        const y = ranges[keyOf.get(ch) ?? 'u0'];
        if (y && !y.log) learnDecimals(state.file.id, ch, y.min, y.max);
      }
    drawAxes(ctx, r, { t0: state.t0, t1: state.t1, left: ranges.u0, right: ranges.u1 }, { ...style, timeLabels: labelled(i) });
    if (main && span > 0) {
      const x: Axis = { a: (2 * main.xScaleNs) / span, b: (2 * Number(main.originNs - state.t0)) / span - 1 };
      for (const ch of chans) {
        const t = main.traces.get(ch);
        const yr = ranges[keyOf.get(ch)!];
        if (!t || t.log !== !!yr.log) continue;
        const y: Axis = { a: 2 / (yr.max - yr.min), b: (-2 * yr.min) / (yr.max - yr.min) - 1 };
        renderer.draw(t.gl, x, y, rgba(colorOf(ch)), r, undefined, clip);
      }
      markers[i] = chans.length === 1 ? drawMarkers(ctx, r, main, chans[0], ranges[keyOf.get(chans[0])!], tk) : 0;
      for (const key of keys) if (axisOf(i, key).range === 'robust') drawClipped(ctx, r, main, chans.filter((ch) => keyOf.get(ch) === key), ranges[key], tk);
    }
    drawMarks(ctx, r, tk);
    drawBuilding(ctx, r, i === 0);
    drawCursors(ctx, r, tk);
    drawPlayhead(ctx, r, tk);
    drawHover(ctx, r, i, chans, tk);
    if (state.box?.pane === i) {
      const [a, b] = [Math.min(state.box.x0, state.box.x1), Math.max(state.box.x0, state.box.x1)];
      ctx.fillStyle = css(tk['--accent'], b - a >= 4 ? 0.16 : 0.07);
      ctx.fillRect(a, r.y, b - a, r.h);
      ctx.strokeStyle = css(tk['--accent'], b - a >= 4 ? 0.8 : 0.35);
      ctx.strokeRect(Math.round(a) + 0.5, r.y + 0.5, Math.round(b - a), r.h - 1);
    }
    ctx.restore();
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

/** Each pane's axes as last drawn. */
const paneY: Record<AxisKey, YAxis>[] = [];

type DrawnTrace = { y: Float32Array; yMin: number; yMax: number };

/** An axis's range in drawn units (log10 for a log axis): its fixed range, or its traces' with some room. */
function axisRange(ax: AxisSettings, ts: DrawnTrace[]): { min: number; max: number } {
  const t = (v: number) => (ax.log ? Math.log10(v) : v);
  if (ax.range === 'fixed' && ax.min != null && ax.max != null && ax.max > ax.min && (!ax.log || ax.min > 0)) return { min: t(ax.min), max: t(ax.max) };
  let lo = Infinity;
  let hi = -Infinity;
  for (const tr of ts) {
    if (Number.isFinite(tr.yMin)) lo = Math.min(lo, tr.yMin);
    if (Number.isFinite(tr.yMax)) hi = Math.max(hi, tr.yMax);
  }
  if (ax.range === 'robust') [lo, hi] = robust(ts) ?? [lo, hi];
  if (!(hi >= lo)) return ax.log ? { min: 0, max: 1 } : { min: -1, max: 1 };
  const zero = ax.range === 'zero' && !ax.log;
  if (zero) [lo, hi] = [Math.min(0, lo), Math.max(0, hi)];
  const pad = (hi - lo) * 0.06 || Math.abs(hi) * 0.06 || (ax.log ? 0.1 : 1);
  return { min: zero && lo === 0 ? 0 : lo - pad, max: zero && hi === 0 ? 0 : hi + pad };
}

/** The 0.1st and 99.9th percentiles of what the traces hold (null when there are too few points). */
function robust(ts: DrawnTrace[]): [number, number] | null {
  const n = ts.reduce((a, t) => a + t.y.length, 0);
  // a sample of at most 100k points is plenty for the percentiles
  const stride = Math.max(1, Math.floor(n / 100_000));
  const vals: number[] = [];
  for (const t of ts) for (let k = 0; k < t.y.length; k += stride) if (Number.isFinite(t.y[k])) vals.push(t.y[k]);
  if (vals.length < 20) return null;
  const sorted = Float64Array.from(vals).sort();
  return [sorted[Math.floor((sorted.length - 1) * 0.001)], sorted[Math.ceil((sorted.length - 1) * 0.999)]];
}

/** Ignore outliers: what's beyond the axis's range is clipped, marked with a small triangle at the edge. */
function drawClipped(ctx: CanvasRenderingContext2D, r: Rect, layer: Layer, chans: number[], ax: YAxis, tk: Record<string, string>): void {
  const span = Number(state.t1 - state.t0);
  const origin = Number(layer.originNs - state.t0);
  ctx.fillStyle = css(tk['--warn'], 0.9);
  let lastX = -Infinity;
  for (const ch of chans) {
    const t = layer.traces.get(ch);
    if (!t) continue;
    for (let k = 0; k < t.y.length; k++) {
      const v = t.y[k];
      if (!(v > ax.max || v < ax.min)) continue;
      const x = r.x + ((origin + t.x[k] * layer.xScaleNs) / span) * r.w;
      if (x < r.x || x > r.x + r.w || x - lastX < 5) continue;
      lastX = x;
      const up = v > ax.max;
      const y = up ? r.y + 1 : r.y + r.h - 1;
      ctx.beginPath();
      ctx.moveTo(x - 3.5, up ? y + 6 : y - 6);
      ctx.lineTo(x + 3.5, up ? y + 6 : y - 6);
      ctx.lineTo(x, y);
      ctx.fill();
    }
  }
}

/** A chart of one channel marks each sample once they are more than this many pixels apart (raw samples only). */
const MARKER_PX = 6;
/** How many markers each pane drew last (for the tests). */
const markers: number[] = [];

/** Each sample as a small ring, as the prototype's, when a one-channel chart is zoomed in that far. */
function drawMarkers(ctx: CanvasRenderingContext2D, r: Rect, layer: Layer, ch: number, ax: YAxis | undefined, tk: Record<string, string>): number {
  const t = layer.traces.get(ch);
  if (!t?.raw || !ax || !(ax.max > ax.min)) return 0;
  const span = Number(state.t1 - state.t0);
  const origin = Number(layer.originNs - state.t0);
  const xs: number[] = [];
  const ys: number[] = [];
  for (let k = 0; k < t.x.length; k++) {
    const x = r.x + ((origin + t.x[k] * layer.xScaleNs) / span) * r.w;
    if (x < r.x - 3 || x > r.x + r.w + 3 || !Number.isFinite(t.y[k])) continue;
    xs.push(x);
    ys.push(r.y + r.h - ((t.y[k] - ax.min) / (ax.max - ax.min)) * r.h);
  }
  if (!xs.length || r.w / xs.length <= MARKER_PX) return 0;
  ctx.lineWidth = 1.5;
  ctx.strokeStyle = colorOf(ch);
  ctx.fillStyle = tk['--plot-bg'];
  for (let k = 0; k < xs.length; k++) {
    ctx.beginPath();
    ctx.arc(xs[k], ys[k], 2.5, 0, 2 * Math.PI);
    ctx.fill();
    ctx.stroke();
  }
  ctx.lineWidth = 1;
  return xs.length;
}

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

/** A table filter's matches (a light shade and a tick along the top) and the rows Apply to charts leaves out (a veil
 * and hatching along the bottom). */
function drawMarks(ctx: CanvasRenderingContext2D, r: Rect, tk: Record<string, string>): void {
  const m = ui.marks;
  if (!m) return;
  const span = Number(state.t1 - state.t0);
  if (span <= 0) return;
  const px = (t: bigint) => r.x + (Number(t - state.t0) / span) * r.w;
  // a run under two pixels wide still shows
  const wide = (a: bigint, b: bigint): [number, number] => {
    const [x0, x1] = [px(a), px(b)];
    return x1 - x0 < 2 ? [(x0 + x1) / 2 - 1, (x0 + x1) / 2 + 1] : [x0, x1];
  };
  ctx.save();
  ctx.beginPath();
  ctx.rect(r.x, r.y, r.w, r.h);
  ctx.clip();
  for (const [a, b] of m.hits) {
    if (b < state.t0 || a > state.t1) continue;
    const [x0, x1] = wide(a, b);
    ctx.fillStyle = css(tk['--fg'], 0.07);
    ctx.fillRect(x0, r.y, x1 - x0, r.h);
    ctx.fillStyle = css(tk['--fg'], 0.6);
    ctx.fillRect(x0, r.y, x1 - x0, 3);
  }
  if (m.excluded.length) {
    if (!hatch) {
      const pc = document.createElement('canvas');
      pc.width = pc.height = 8;
      const g = pc.getContext('2d')!;
      g.strokeStyle = 'rgba(128,128,128,.7)';
      g.lineWidth = 1.5;
      g.beginPath();
      g.moveTo(0, 8);
      g.lineTo(8, 0);
      g.stroke();
      hatch = ctx.createPattern(pc, 'repeat');
    }
    for (const [a, b] of m.excluded) {
      if (b < state.t0 || a > state.t1) continue;
      const [x0, x1] = wide(a, b);
      ctx.fillStyle = css(tk['--fg-muted'], 0.06);
      ctx.fillRect(x0, r.y, x1 - x0, r.h);
      if (hatch) {
        ctx.fillStyle = hatch;
        ctx.fillRect(x0, r.y + r.h - 7, x1 - x0, 7);
      }
    }
  }
  ctx.restore();
}
let hatch: CanvasPattern | null = null;

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
function drawHover(ctx: CanvasRenderingContext2D, r: Rect, i: number, chans: number[], tk: Record<string, string>): void {
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
    const y = paneY[i]?.[axisKey(i, ch)];
    const v = y?.log ? Math.log10(s.v) : s.v;
    if (!y || !Number.isFinite(v)) continue;
    const py = r.y + r.h - ((v - y.min) / (y.max - y.min)) * r.h;
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

/** What the overview drew last, per channel: columns with values, and those whose band has a height (for the tests). */
const ovDrawn: { ch: number; cols: number; band: number }[] = [];

/** The overview as the prototype draws it: per pixel column each channel's min/max band, filled, with its middle as
 *  a solid line on top (a smooth channel's band is a hairline, so the line keeps it visible); each channel scaled to
 *  its own range. */
function drawOverviewBands(ctx: CanvasRenderingContext2D, r: Rect, ov: Layer, t0: bigint, span: number): void {
  const n = ov.traces.size;
  ovDrawn.length = 0;
  const origin = Number(ov.originNs - t0);
  const cols = Math.max(1, Math.round(r.w));
  ctx.save();
  ctx.beginPath();
  ctx.rect(r.x, r.y, r.w, r.h);
  ctx.clip();
  for (const [ch, t] of ov.traces) {
    if (!(t.yMax >= t.yMin)) continue;
    // each column's lowest and highest value (NaN: nothing there)
    const lo = new Float64Array(cols).fill(NaN);
    const hi = new Float64Array(cols).fill(NaN);
    for (let k = 0; k < t.x.length; k++) {
      const v = t.y[k];
      if (!Number.isFinite(v)) continue;
      const c = Math.floor(((origin + t.x[k] * ov.xScaleNs) / span) * cols);
      if (c < 0 || c >= cols) continue;
      if (!(lo[c] <= v)) lo[c] = v;
      if (!(hi[c] >= v)) hi[c] = v;
    }
    const range = t.yMax - t.yMin || 1;
    const Y = (v: number) => r.y + r.h * (0.94 - (0.88 * (v - t.yMin)) / range);
    ovDrawn.push({ ch, cols: lo.filter((v) => !Number.isNaN(v)).length, band: lo.filter((v, c) => hi[c] > v).length });
    const X = (c: number) => r.x + ((c + 0.5) / cols) * r.w;
    const color = colorOf(ch);
    // the band: one closed shape per run of columns with values
    ctx.fillStyle = css(color, n > 1 ? 0.32 : 0.45);
    ctx.beginPath();
    for (let c = 0; c < cols; ) {
      if (Number.isNaN(lo[c])) {
        c++;
        continue;
      }
      const start = c;
      ctx.moveTo(X(c), Y(hi[c]));
      while (c + 1 < cols && !Number.isNaN(lo[c + 1])) ctx.lineTo(X(++c), Y(hi[c]));
      for (let k = c; k >= start; k--) ctx.lineTo(X(k), Y(lo[k]));
      ctx.closePath();
      c++;
    }
    ctx.fill();
    // the middle
    ctx.strokeStyle = css(color, n > 1 ? 0.85 : 0.95);
    ctx.lineWidth = 1.25;
    ctx.lineJoin = 'round';
    ctx.beginPath();
    let pen = false;
    for (let c = 0; c < cols; c++) {
      if (Number.isNaN(lo[c])) {
        pen = false;
        continue;
      }
      const [x, y] = [X(c), Y((lo[c] + hi[c]) / 2)];
      if (pen) ctx.lineTo(x, y);
      else ctx.moveTo(x, y);
      pen = true;
    }
    ctx.stroke();
  }
  ctx.restore();
  ctx.lineWidth = 1;
}

/** The channels the overview draws: the one it is pinned to, else the selected pane's. */
function ovChans(): number[] {
  const n = state.file?.info.channels.length ?? 0;
  const pin = settings.overviewCh;
  return pin != null && pin < n ? [pin] : (panes()[stack.sel] ?? []);
}

function drawOverview(ctx: CanvasRenderingContext2D, tk: Record<string, string>): void {
  const f = state.file;
  const r = ovRect();
  if (!f || !r) return;
  const [t0, t1] = [f.first, f.last + 1n];
  const span = Number(t1 - t0);
  const ov = state.ov;
  if (ov && span > 0) drawOverviewBands(ctx, r, ov, t0, span);
  drawBuilding(ctx, r, false, t0, t1);
  drawPlayhead(ctx, r, tk, t0, t1);
  // the window on the current view
  const xa = Math.max(r.x, r.x + (Number(state.t0 - t0) / span) * r.w);
  const xb = Math.min(r.x + r.w, r.x + (Number(state.t1 - t0) / span) * r.w);
  ctx.fillStyle = css(tk['--plot-bg'], 0.55);
  ctx.fillRect(r.x, r.y, Math.max(0, xa - r.x), r.h);
  ctx.fillRect(xb, r.y, Math.max(0, r.x + r.w - xb), r.h);
  ctx.strokeStyle = tk['--accent'];
  ctx.strokeRect(Math.round(xa) + 0.5, r.y + 0.5, Math.max(2, Math.round(xb - xa) - 1), r.h - 1);
  if (state.ovBox) {
    ctx.fillStyle = css(tk['--accent'], 0.25);
    ctx.fillRect(Math.min(state.ovBox.x0, state.ovBox.x1), r.y, Math.abs(state.ovBox.x1 - state.ovBox.x0), r.h);
  }
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
  scheduleDraw();
}

function renderChannels(): void {
}

function renderOverviewHead(): void {
  const slot = settings.overviewTop ? $('ovTop') : $('ovBottom');
  $('ovTop').replaceChildren();
  $('ovBottom').replaceChildren();
  // a live tab has no overview: the live strip's scrubber spans the session
  if (!state.file || live.on) return;
  const wrap = document.createElement('div');
  wrap.className = 'overview';
  const f = state.file;
  const chs = f.info.channels;
  const pin = settings.overviewCh != null && settings.overviewCh < chs.length ? settings.overviewCh : null;
  const n = panes()[stack.sel]?.length ?? 0;
  const rangeTxt = `${timeLabel(f.first, Number(f.last - f.first) / 4)} – ${timeLabel(f.last, Number(f.last - f.first) / 4)}`;
  wrap.innerHTML =
    `<div class="ovhead"><span>Overview of</span><select class="sel ovsel" data-ovch aria-label="What the overview shows"><option value=""${pin == null ? ' selected' : ''}>pane ${stack.sel + 1} · ${n} ${n === 1 ? 'channel' : 'channels'}</option>` +
    chs.map((c, k) => `<option value="${k}"${pin === k ? ' selected' : ''}>${esc(c.name)}</option>`).join('') +
    `</select><span class="spacer"></span><span class="num ovrange" data-ovrange>${esc(rangeTxt)}</span><button class="ovsw" role="switch" aria-checked="${settings.overviewTop}" data-ovtop><span class="knob"></span>Show on top</button></div><div class="ovplot"></div>`;
  slot.append(wrap);
}

/** Words for where values are read: under the pointer, at cursor A, or at the end of the view. */
function setValuesAt(): void {
  const f = state.file;
  const span = Number(state.t1 - state.t0);
  if (!f) ui.valuesAt = '';
  else if (state.hoverT != null) ui.valuesAt = `at the pointer · ${readoutTime(state.hoverT, span)}`;
  else if (state.A != null) ui.valuesAt = `at cursor A · ${readoutTime(state.A, span)}`;
  else ui.valuesAt = `at the end of the view · ${readoutTime(state.t1 > f.last ? f.last : state.t1, span)}`;
}

function updateChips(): void {
  setValuesAt();
  const values: Record<number, string> = {};
  state.hover.forEach((s, ch) => (values[ch] = fmtCh(state.file?.id ?? null, ch, s.v)));
  if (JSON.stringify(values) !== JSON.stringify(stack.values)) stack.values = values;
  ui.pointer = state.hoverT != null ? `pointer ${readoutTime(state.hoverT, Number(state.t1 - state.t0))}` : '';
}

function saveLayout(): void {
  // a workspace tab's edits are its unsaved changes, not the file's layout
  if (!state.file || frontTab()?.ws) return;
  settings.layouts[state.file.info.path] = stack.panes.map((p) => ({ ...p, channels: [...p.channels] }));
  saveSettings();
}

/** After an edit of the panes, once they are on the page (their plots are measured). */
async function layoutChanged(): Promise<void> {
  saveLayout();
  markDirty();
  await tick();
  renderPanes();
  overviewChanged();
  if (state.A != null && state.B != null) void refreshMeasure();
  void requestResting();
}

function selectPane(i: number): void {
  if (i === stack.sel) return;
  stack.sel = i;
  renderChannels();
  renderOverviewHead();
  overviewChanged();
}

/** A new file's panes: one per channel up to four, else the channels shared out over four. */
/** The panes a file opens with, by the starter chosen in Settings. */
function startPanes(n: number, kind: StarterPanes): Pane[] {
  const all = Array.from({ length: n }, (_, i) => i);
  if (kind === 'none') return [];
  if (kind === 'table') return [{ channels: all, mode: 'table' }];
  if (kind === 'values') return [{ channels: all, mode: 'values' }];
  const ps = defaultPanes(n);
  return kind === 'starterboth' ? ps.map((p) => ({ ...p, mode: 'both' as const })) : ps;
}

/** A file's panes: those it was left with (when remembering and it has some that fit), else a starter. */
function panesFor(path: string, n: number): Pane[] {
  const saved = settings.layouts[path];
  if (settings.fileOpen === 'remember' && saved?.every((p) => p.channels?.every((ch) => ch < n))) return copyPanes(saved);
  return startPanes(n, settings.fileOpen === 'remember' ? settings.fileOpenNew : settings.fileOpen);
}

function defaultPanes(n: number): Pane[] {
  if (n <= 4) return Array.from({ length: n }, (_, i) => ({ channels: [i] }));
  const per = Math.ceil(n / 4);
  return Array.from({ length: Math.ceil(n / per) }, (_, p) => ({
    channels: Array.from({ length: Math.min(per, n - p * per) }, (_, k) => p * per + k),
  }));
}

// ---------- Measure ----------

let measureSeq = 0;
/** The channel the Stats panel is about, and which Measure panels are open (kept across redraws). */
let measureCh: number | null = null;
let measureTimer = 0;
/** With no cursors, Measure's stats follow the view: refresh a moment after it stops moving. */
function scheduleMeasure(): void {
  if (!measureOpen()) return;
  clearTimeout(measureTimer);
  measureTimer = window.setTimeout(() => void refreshMeasure(), 150);
}

async function refreshMeasure(): Promise<void> {
  const f = state.file;
  const seq = ++measureSeq;
  scheduleSessionSave();
  ui.anyCursor = state.A != null || state.B != null;
  ui.cursors = state.A != null && state.B != null;
  play.canLoop = ui.cursors;
  if (!ui.cursors) play.loop = false;
  if (!f) {
    fillMeasure(`<p class="mnote">Open a recording to measure it.</p>`, '');
    return;
  }
  const chans = plotted();
  if (measureCh == null || !chans.includes(measureCh)) measureCh = chans[0] ?? null;
  const both = ui.cursors;
  const [a, b] = both ? (state.A! <= state.B! ? [state.A!, state.B!] : [state.B!, state.A!]) : [state.t0, state.t1 - 1n];
  const at = (t: bigint | null) => (t != null && chans.length ? invoke<Nearest[]>('values_at', { file: f.id, channels: chans, t: t.toString() }) : Promise.resolve([] as Nearest[]));
  const t0 = performance.now();
  const [va, vb, st] = await Promise.all([
    at(state.A),
    at(state.B),
    measureCh != null ? invoke<ChannelStats[] | null>('stats', { file: f.id, channels: [measureCh], span: { t0: a.toString(), t1: (b + 1n).toString() } }) : Promise.resolve(null),
  ]);
  const ms = performance.now() - t0;
  if (seq !== measureSeq) return;
  const span = Number(state.t1 - state.t0);
  const dt = state.A != null && state.B != null ? Number(state.B - state.A) : null;
  const val = (list: Nearest[], ch: number) => list.find((n) => n.channel === ch)?.value ?? null;
  const fv = (ch: number, v: number | null | undefined) => fmtCh(f.id, ch, v);
  const tile = (k: string, v: string, hi = false) => `<div class="mt${hi ? ' hi' : ''}"><span class="k">${k}</span><span class="v num">${v}</span></div>`;
  const rowsHtml = chans.length
    ? chans
        .map((ch) => {
          const [ya, yb] = [val(va, ch), val(vb, ch)];
          return `<tr data-mrow="${ch}"><td><span class="sw8" style="background:${colorOf(ch)}"></span>${esc(f.info.channels[ch].name)}</td><td>${fv(ch, ya)}</td><td>${fv(ch, yb)}</td><td>${ya != null && yb != null ? fv(ch, yb - ya) : '—'}</td></tr>`;
        })
        .join('')
    : `<tr><td colspan="4" class="mnote">Plot a channel to read values here.</td></tr>`;
  const c = measureCh != null ? f.info.channels[measureCh] : null;
  const s = st?.[0];
  const u = c?.unit ? ` ${esc(c.unit)}` : '';
  const sd = s?.std ?? null;
  const rms = s?.mean != null && sd != null ? Math.sqrt(s.mean * s.mean + sd * sd) : null;
  const building = Math.floor((100 * (state.building?.done ?? 0)) / Math.max(1, state.building?.total ?? 1));
  const stat = (k: string, v: string) => `<div class="mrow"><span class="k">${k}</span><span class="v num">${v}</span></div>`;
  const statsBody =
    c == null
      ? `<p class="mnote">Plot a channel to see its stats.</p>`
      : st == null
        ? `<p class="mnote">Reading ${esc(nameOf(f.info.path))}… exact stats once it's read (${building}%).</p>`
        : stat('Samples', s ? s.count.toLocaleString() : '—') +
          stat('Min', s?.min != null ? fv(measureCh!, s.min) + u : '—') +
          stat('Max', s?.max != null ? fv(measureCh!, s.max) + u : '—') +
          stat('Mean', s?.mean != null ? fv(measureCh!, s.mean) : '—') +
          stat('RMS', rms != null ? fv(measureCh!, rms) : '—') +
          stat('Std dev', sd != null ? fv(measureCh!, sd) : '—') +
          stat('Computed in', `${ms.toFixed(ms < 10 ? 1 : 0)} ms`);
  const pick = `<select class="sel" data-mch aria-label="Channel for the stats">${chans.map((ch) => `<option value="${ch}"${ch === measureCh ? ' selected' : ''}>${esc(f.info.channels[ch].name)}</option>`).join('')}</select>`;
  ui.statsTitle = `Stats · ${both ? 'between cursors' : 'visible range'}`;
  fillMeasure(
    (state.A == null && state.B == null ? `<p class="mnote howto">Place cursors A and B: press C, then click a chart twice. A and B also place them at the pointer.</p>` : '') +
      `<div class="mtiles">${tile('A', state.A != null ? readoutTime(state.A, span) : '—')}${tile('B', state.B != null ? readoutTime(state.B, span) : '—')}${tile('Δt', dt != null ? duration(dt) : '—', true)}${tile('1 / Δt', dt ? frequency(1e9 / Math.abs(dt)) : '—')}</div>` +
      `<table class="mtab"><thead><tr><th>Channel</th><th>A</th><th>B</th><th>Δ</th></tr></thead><tbody>${rowsHtml}</tbody></table>`,
    (chans.length ? `<div class="mpick">${pick}</div>` : '') + statsBody,
  );
}

/** Measure's panels' bodies, in whichever sidebar they sit (absent while their sidebar is closed). */
function fillMeasure(cur: string, stats: string): void {
  const [c, s] = [document.getElementById('mpCur'), document.getElementById('mpStats')];
  if (c) c.innerHTML = cur;
  if (s) s.innerHTML = stats;
}
/** Measure shows somewhere: one of its panels is open in a sidebar that's shown. */
const measureOpen = () => [...document.querySelectorAll<HTMLElement>('#mpCur, #mpStats')].some((e) => !e.hidden && !!e.offsetParent);

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

// ---------- tabs: one per open file, and workspace tabs on them ----------

/** A tab's state while another tab is in front (the one in front lives in `state` and `stack`). */
interface TabData {
  file: NonNullable<typeof state.file>;
  building: typeof state.building;
  panes: Pane[];
  sel: number;
  t0: bigint;
  t1: bigint;
  A: bigint | null;
  B: bigint | null;
  history: [bigint, bigint][];
  future: [bigint, bigint][];
  exactMs: number;
  /** Its transport when it was left (restored paused), or null in Analyze. */
  play: null | { t: number; speed: number; win: number; follow: boolean; loop: boolean };
  /** A live tab: whether it followed now, its window, and its source's last health. */
  live: null | { following: boolean; win: number; health: Health | null };
}
const tabData = new Map<number, TabData>();
let nextTab = 1;
const copyPanes = (ps: Pane[]): Pane[] => ps.map((p) => ({ ...p, channels: [...p.channels] }));
const nameOf = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** Keeps the tab in front's state, to come back to. */
function stashTab(): void {
  const f = state.file;
  if (tabs.active < 0 || !f) return;
  tabData.set(tabs.active, {
    file: f,
    building: state.building,
    panes: copyPanes(stack.panes),
    sel: stack.sel,
    t0: state.t0,
    t1: state.t1,
    A: state.A,
    B: state.B,
    history: state.history,
    future: state.future,
    exactMs: state.exactMs,
    play: play.open ? { t: play.t, speed: play.speed, win: play.win, follow: play.follow, loop: play.loop } : null,
    live: live.on ? { following: live.following, win: live.win, health: live.health } : null,
  });
}

function freeLayers(): void {
  state.layers.forEach((l) => l?.traces.forEach((t) => renderer.free(t.gl)));
  state.ov?.traces.forEach((t) => renderer.free(t.gl));
  Object.assign(state, { layers: [], ov: null, ovStale: true, coarseSince: [], hatchFrom: null, hoverT: null });
  state.hover.clear();
  stack.values = {};
}

/** Brings tab `key` to the front, as it was left. */
async function showTab(key: number): Promise<void> {
  const d = tabData.get(key);
  if (!d || (key === tabs.active && state.file)) return;
  stashTab();
  tabs.active = key;
  freeLayers();
  Object.assign(state, {
    file: d.file,
    building: d.building,
    A: d.A,
    B: d.B,
    history: d.history,
    future: d.future,
    exactMs: d.exactMs,
  });
  stack.panes = copyPanes(d.panes);
  stack.sel = d.sel;
  normalize();
  closePlay();
  Object.assign(live, { on: !!d.live, following: d.live?.following ?? true, win: d.live?.win ?? 30, sources: false, health: d.live?.health ?? null });
  ui.file = nameOf(d.file.info.path);
  ui.path = d.file.info.path;
  ui.time = fileTime(d.file.info.time.kind === 'index', d.file.first);
  ui.measureDismissed = false;
  ui.channels = d.file.info.channels.map((c) => ({ name: c.name, unit: c.unit }));
  $('empty').hidden = true;
  saveSession();
  await tick();
  renderPanes();
  liveDriving = true;
  setView(d.t0, d.t1);
  liveDriving = false;
  if (live.on && live.following) liveActions.jump();
  if (live.on) void pollLive();
  if (d.play) {
    const end = Number(d.file.last - d.file.first) / 1e9;
    Object.assign(play, { ...d.play, open: true, running: false, end, canLoop: d.A != null && d.B != null });
  }
  void refreshMeasure();
}

/** Opens `path` in a tab of its own (or brings its tab to the front). */
async function openPath(path: string): Promise<void> {
  const open = tabs.list.find((t) => t.path === path && !t.ws);
  if (open) return showTab(open.key);
  // a file a workspace tab shows is open already: its file tab shares it
  const shown = tabs.list.find((t) => t.path === path && !t.live);
  const again = shown ? fileOf(shown.key) : null;
  try {
    const key = nextTab++;
    let file: OpenFile;
    let building: TabData['building'];
    if (again && shown) {
      file = again;
      building = shown.key === tabs.active ? state.building : (tabData.get(shown.key)?.building ?? null);
    } else {
      const progress = new Channel<Progress>();
      // progress is the file's, for every tab on it; any that comes before the reply waits for its id
      let id: number | null = null;
      let early: Progress | null = null;
      progress.onmessage = (p) => (id == null ? (early = p) : onProgress(id, p));
      const o = await invoke<Opened>('open', { path, progress });
      id = o.id;
      if (early) onProgress(id, early);
      if (!o.range) {
        void invoke('close', { file: o.id });
        throw new Error('it has no samples');
      }
      file = { id: o.id, info: o.info, first: BigInt(o.range[0]), last: BigInt(o.range[1]) };
      building = o.status.state === 'built' ? null : { done: 0, total: o.info.samples, until: null };
    }
    const n = file.info.channels.length;
    tabData.set(key, {
      file,
      building,
      panes: panesFor(file.info.path, n),
      sel: 0,
      t0: file.first,
      t1: file.last + 1n,
      A: null,
      B: null,
      history: [],
      future: [],
      exactMs: Infinity,
      play: null,
      live: null,
    });
    tabs.list.push({ key, path: file.info.path, name: nameOf(file.info.path) });
    ui.start = false;
    setSetting('recent', [file.info.path, ...settings.recent.filter((r) => r !== file.info.path)].slice(0, 8));
    ui.note = '';
    ui.undo = null;
    await showTab(key);
  } catch (e) {
    note(`Couldn't open ${nameOf(path)}: ${e}`);
  }
}

// ---------- Live: a tab whose file grows, its view following now ----------

let liveDriving = false;
/** When the source was last asked how far it has recorded. */
let livePolledAt = 0;
let liveRaf = 0;

/** Connects a live source and opens it in a tab of its own, following now. */
async function openLive(endpoint: string, descriptor: string): Promise<void> {
  live.connecting = true;
  live.error = '';
  try {
    const o = await invoke<Opened>('live_connect', { endpoint, descriptor: descriptor || null });
    if (!o.range) throw new Error('no samples yet');
    const key = nextTab++;
    const file = { id: o.id, info: o.info, first: BigInt(o.range[0]), last: BigInt(o.range[1]) };
    tabData.set(key, {
      file,
      building: null,
      panes: startPanes(o.info.channels.length, settings.fileOpenNew),
      sel: 0,
      t0: file.last - 30_000_000_000n,
      t1: file.last + 1n,
      A: null,
      B: null,
      history: [],
      future: [],
      exactMs: 0,
      play: null,
      live: { following: true, win: 30, health: null },
    });
    const name = endpoint.replace(/^file:\/\//, '').split(/[\\/]/).pop() || endpoint;
    tabs.list.push({ key, path: endpoint, name, live: true });
    live.connect = false;
    await showTab(key);
    void pollLive();
  } catch (e) {
    live.error = `Couldn't connect: ${e}`;
  } finally {
    live.connecting = false;
  }
}

interface LiveState {
  range: [string, string] | null;
  samples: number;
  info: RecordingInfo;
  health: Health;
  session: string;
}

/** Asks the source in front how far it has recorded (the view follows that). */
async function pollLive(): Promise<void> {
  const f = state.file;
  if (!live.on || !f) return;
  try {
    const st = await invoke<LiveState>('live_state', { file: f.id });
    if (state.file !== f || !st.range) return;
    if (st.info.channels.length !== f.info.channels.length) ui.channels = st.info.channels.map((c) => ({ name: c.name, unit: c.unit }));
    f.info = st.info;
    f.first = BigInt(st.range[0]);
    f.last = BigInt(st.range[1]);
    livePolledAt = performance.now();
    live.recorded = Number(f.last - f.first) / 1e9;
    live.health = st.health;
    live.session = st.session;
    if (!live.following) live.back = Math.max(0, Number(f.last - state.t1) / 1e9);
    if (live.following && !liveRaf) liveRaf = requestAnimationFrame(followLive);
  } catch {
    /* the tab closed meanwhile */
  }
}
setInterval(() => void pollLive(), 100);

/** Each frame while following: the view's end at now (the last recorded time, moved on by the time since it was
 * asked, at most 0.15 s), over the window. */
function followLive(now: number): void {
  liveRaf = 0;
  const f = state.file;
  if (!live.on || !live.following || !f) return;
  if (live.health?.ended) return followTo(f.last + 1n);
  const ahead = BigInt(Math.round(Math.min(150, Math.max(0, now - livePolledAt)) * 1e6));
  followTo(f.last + 1n + ahead);
  liveRaf = requestAnimationFrame(followLive);
}

function followTo(end: bigint): void {
  liveDriving = true;
  setView(end - BigInt(Math.round(live.win * 1e9)), end);
  liveDriving = false;
  live.back = 0;
}

Object.assign(liveActions, {
  connect: (endpoint: string, descriptor: string) => void openLive(endpoint, descriptor),
  setWindow: (w: number) => {
    live.win = w;
    const f = state.file;
    if (!f) return;
    if (live.following) return followTo(f.last + 1n);
    liveDriving = true;
    setView(state.t1 - BigInt(Math.round(w * 1e9)), state.t1);
    liveDriving = false;
  },
  jump: () => {
    live.following = true;
    if (!liveRaf) liveRaf = requestAnimationFrame(followLive);
  },
  scrub: (fraction: number) => {
    const f = state.file;
    if (!f) return;
    if (fraction >= 0.999) return liveActions.jump();
    live.following = false;
    const end = f.first + BigInt(Math.round(Number(f.last - f.first) * fraction));
    liveDriving = true;
    setView(end - BigInt(Math.round(live.win * 1e9)), end);
    liveDriving = false;
    live.back = Math.max(0, Number(f.last - end) / 1e9);
  },
  stop: async () => {
    const f = state.file;
    if (!f) return;
    live.health = await invoke<Health>('live_stop', { file: f.id });
  },
});

/** Closes tab `key` and its file; the next tab comes to the front, or the window empties. */
async function closeTab(key: number): Promise<void> {
  const i = tabs.list.findIndex((t) => t.key === key);
  if (i < 0) return;
  const front = key === tabs.active;
  const file = fileOf(key);
  const shared = tabs.list.some((t) => t.key !== key && fileOf(t.key)?.id === file?.id);
  if (file && !shared) void invoke('close', { file: file.id });
  tabs.list.splice(i, 1);
  tabData.delete(key);
  wsBase.delete(key);
  if (front) {
    tabs.active = -1;
    live.on = false;
    const next = tabs.list[Math.min(i, tabs.list.length - 1)];
    if (next) await showTab(next.key);
    else emptyWindow();
  }
  saveSession();
}

/** While a session is being put away or restored, tabs come and go without saving it. */
let sessionBusy = false;
let sessionTimer = 0;
/** The view and cursors move often: save the session a moment after they stop. */
function scheduleSessionSave(): void {
  clearTimeout(sessionTimer);
  sessionTimer = window.setTimeout(saveSession, 400);
}

/** The folder's session (its tabs, open files, each file's view and cursors), to reopen with the folder. */
function saveSession(): void {
  if (sessionBusy) return;
  clearTimeout(sessionTimer);
  const front = frontTab();
  const active = front?.path ?? null;
  const files = [...new Set(tabs.list.filter((t) => !t.live).map((t) => t.path))];
  const session: Session = { files, active: active && files.includes(active) ? active : null };
  const kept = tabs.list.filter((t) => !t.live);
  const ws = kept.flatMap((t, at) => (t.ws ? [{ path: t.path, name: t.ws, at, ...(t.shared ? { shared: t.shared } : {}) }] : []));
  if (ws.length) session.ws = ws;
  if (front?.ws) session.activeWs = front.ws;
  // each file tab's time range and cursors
  const views: NonNullable<Session['views']> = {};
  for (const t of kept) {
    if (t.ws) continue;
    const d = t.key === tabs.active ? state : tabData.get(t.key);
    if (!d || !d.t1) continue;
    views[t.path] = { t0: String(d.t0), t1: String(d.t1), ...(d.A != null ? { A: String(d.A) } : {}), ...(d.B != null ? { B: String(d.B) } : {}) };
  }
  if (Object.keys(views).length) session.views = views;
  session.at = Date.now();
  setSetting('sessions', { ...settings.sessions, [settings.folder]: session });
}

/** Opens a session's tabs (and `front`, a file asked for, in front), each file at its view with its cursors. */
async function restoreSession(s: Session | undefined, front: string | null = null): Promise<void> {
  sessionBusy = true;
  try {
    const { files = [], active = null, ws, activeWs, views } = s ?? {};
    const at = (path: string) => {
      const v = views?.[path];
      if (!v || state.file?.info.path !== path) return;
      setView(BigInt(v.t0), BigInt(v.t1));
      state.A = v.A != null ? BigInt(v.A) : null;
      state.B = v.B != null ? BigInt(v.B) : null;
      scheduleDraw();
      void refreshMeasure();
    };
    for (const f of files) {
      await openPath(f);
      at(f);
    }
    if (ws?.some((w) => w.shared)) await loadShared().catch(() => {});
    for (const w of ws ?? []) {
      await openPath(w.path);
      if (w.shared) await openShared(w.shared);
      else await openWs(w.name);
      // back where it was among the tabs
      const k = tabs.list.findIndex((t) => t.ws === w.name && t.shared === w.shared);
      if (k >= 0 && k !== w.at) tabs.list.splice(w.at, 0, ...tabs.list.splice(k, 1));
    }
    const wsFront = !front && activeWs ? tabs.list.find((t) => t.ws === activeWs) : null;
    if (wsFront) await showTab(wsFront.key);
    else if (front ?? active) await openPath((front ?? active)!);
  } finally {
    sessionBusy = false;
  }
  saveSession();
}

/** Puts the folder's session away (its tabs close quietly; live sources keep going), then opens folder `path`'s. */
async function switchSession(path: string, list: () => Promise<void>): Promise<void> {
  saveSession();
  sessionBusy = true;
  try {
    for (const t of tabs.list.filter((t) => !t.live)) await closeTab(t.key);
    await list();
  } finally {
    sessionBusy = false;
  }
  await restoreSession(settings.sessions[path]);
  ui.note = '';
}

// ---------- Workspaces: saved layouts, each shown in a tab of its own on the file in front ----------

const frontTab = () => tabs.list.find((t) => t.key === tabs.active);
/** The file tab `key` shows. */
const fileOf = (key: number) => (key === tabs.active ? state.file : tabData.get(key)?.file) ?? null;
const channelNames = () => state.file?.info.channels.map((c) => c.name) ?? [];
const findWs = (name: string) => settings.workspaces.find((w) => w.name === name);
/** The front tab's panes as a layout's, to compare with the layout as loaded or saved. */
const wsSnapshot = () => JSON.stringify(toWorkspace('', stack.panes, channelNames()).panes);
/** Each workspace tab's panes as loaded or last saved. */
const wsBase = new Map<number, string>();

function markDirty(): void {
  const t = frontTab();
  if (!t?.ws) return;
  const dirty = wsSnapshot() !== wsBase.get(t.key);
  if (!!t.dirty !== dirty) t.dirty = dirty;
}

function storeWs(ws: Workspace, at?: number): void {
  const i = settings.workspaces.findIndex((w) => w.name === ws.name);
  if (i >= 0) settings.workspaces[i] = ws;
  else settings.workspaces.splice(at ?? settings.workspaces.length, 0, ws);
  saveSettings();
}

function nameError(name: string, own?: string): string | null {
  if (!name) return 'Give the layout a name.';
  if (name !== own && findWs(name)) return `There is already a layout called "${name}".`;
  return null;
}

/** Brings tab `key` to the front, then runs `f` there. */
async function atTab(key: number, f: () => void | Promise<void>): Promise<void> {
  if (key !== tabs.active) await showTab(key);
  if (key === tabs.active) await f();
}

/** Save as…: a layout of tab `key`'s panes, which the tab then shows. */
function saveAs(key: number, raw: string): string | null {
  const name = raw.trim();
  const t = tabs.list.find((x) => x.key === key);
  const err = t ? nameError(name, t.shared ? undefined : t.ws) : 'That tab is closed.';
  if (err || !t) return err;
  void atTab(key, () => {
    storeWs(toWorkspace(name, stack.panes, channelNames()));
    Object.assign(t, { ws: name, name, dirty: false, shared: undefined });
    wsBase.set(key, wsSnapshot());
    saveSession();
    note(`Saved layout "${name}".`);
  });
  return null;
}

function saveWs(key: number): void {
  const t = tabs.list.find((x) => x.key === key);
  if (!t?.ws || t.shared) return;
  void atTab(key, () => {
    storeWs(toWorkspace(t.ws!, stack.panes, channelNames()));
    wsBase.set(key, wsSnapshot());
    t.dirty = false;
  });
}

function revertWs(key: number): void {
  const t = tabs.list.find((x) => x.key === key);
  const ws = t?.shared ? ui.shared.find((v) => v.file === t.shared)?.ws : t?.ws ? findWs(t.ws) : null;
  if (!t || !ws) return;
  void atTab(key, () => {
    stack.panes = fromWorkspace(ws, channelNames()).panes;
    stack.sel = 0;
    normalize();
    void layoutChanged();
  });
}

/** Deletes one of your layouts and closes its tabs, with Undo. */
function deleteWs(name: string): void {
  const at = settings.workspaces.findIndex((w) => w.name === name);
  if (at < 0) return;
  const [gone] = settings.workspaces.splice(at, 1);
  saveSettings();
  void (async () => {
    for (const t of tabs.list.filter((t) => t.ws === name && !t.shared)) await closeTab(t.key);
    note(`Deleted layout "${name}".`, () => storeWs(gone, at));
  })();
}

/** Opens layout `name` in a tab of its own on the file in front (or brings its tab forward). */
async function openWs(name: string): Promise<void> {
  const open = tabs.list.find((t) => t.ws === name && !t.shared);
  if (open) return showTab(open.key);
  const ws = findWs(name);
  if (ws) await openLayout(ws);
}

/** Shows `ws` in a new tab on the file in front; a shared view's (`shared`) also sets its time range. */
async function openLayout(ws: Workspace, shared?: string): Promise<void> {
  const f = state.file;
  if (!f || live.on) {
    note(`Open a recording first: "${ws.name}" shows on the file in front.`);
    return;
  }
  stashTab();
  const cur = tabData.get(tabs.active)!;
  const { panes, missing } = fromWorkspace(ws, channelNames());
  const key = nextTab++;
  const at = (s: number) => f.first + BigInt(Math.round(s * 1e9));
  const view = shared && ws.view && ws.view[1] > ws.view[0] ? { t0: at(ws.view[0]), t1: at(ws.view[1]) } : {};
  tabData.set(key, { ...cur, panes, sel: 0, history: [], future: [], play: null, live: null, ...view });
  tabs.list.push({ key, path: f.info.path, name: ws.name, ws: ws.name, shared });
  await showTab(key);
  wsBase.set(key, wsSnapshot());
  if (missing.length) {
    const list = missing.slice(0, 3).join(', ') + (missing.length > 3 ? ', …' : '');
    note(`${nameOf(f.info.path)} has no ${missing.length === 1 ? 'channel' : 'channels'} ${list}: ${missing.length === 1 ? 'it stays' : 'they stay'} out of "${ws.name}".`);
  }
}

// ---------- shared views: `.tlmbench/views/` in Explorer's folder ----------

const sep = (path: string) => (path.includes('\\') && !path.includes('/') ? '\\' : '/');

/** The folder's shared views, read again (the + menu does this as it opens). */
async function loadShared(): Promise<void> {
  const folder = settings.folder;
  if (!folder) {
    ui.shared = [];
    return;
  }
  const got = await invoke<{ file: string; view: unknown; error: string | null }[]>('shared_views', { folder });
  if (folder !== settings.folder) return;
  ui.shared = got.map((v) => {
    const ok = isWorkspace(v.view);
    return { file: v.file, ws: ok ? (v.view as Workspace) : null, error: ok ? null : (v.error ?? 'not a layout') };
  });
}

/** Opens a shared view: its primary file (from the folder), then the layout on it with its time range. */
async function openShared(file: string): Promise<void> {
  const open = tabs.list.find((t) => t.shared === file);
  if (open) return showTab(open.key);
  const ws = ui.shared.find((v) => v.file === file)?.ws;
  if (!ws) return;
  const primary = ws.files?.find((f) => f.slot === 1);
  if (primary) {
    const s = sep(settings.folder);
    const path = settings.folder.replace(/[\\/]$/, '') + s + primary.path.split('/').join(s);
    await openPath(path);
    if (state.file?.info.path !== path) return;
  }
  await openLayout(ws, file);
}

/** Tab `key`'s layout as a shared view: its file relative to the folder and the time range in view. */
function sharedView(name: string): Workspace | string {
  const f = state.file;
  const folder = settings.folder.replace(/[\\/]$/, '');
  if (!f) return 'Open a recording first.';
  const s = sep(folder);
  if (!folder || !f.info.path.startsWith(folder + s)) return `Share writes into the folder Explorer shows, and ${nameOf(f.info.path)} isn't in it.`;
  const secs = (t: bigint) => Number(t - f.first) / 1e9;
  return {
    ...toWorkspace(name, stack.panes, channelNames()),
    files: [{ path: f.info.path.slice(folder.length + 1).split(s).join('/'), slot: 1 }],
    view: [secs(state.t0), secs(state.t1)],
  };
}

/** Share…: writes tab `key`'s layout to the folder's `.tlmbench/views/`; a folder that can't be written offers
 * Export file… instead. */
function share(key: number, raw: string): string | null {
  const name = raw.trim();
  if (!name) return 'Give the view a name.';
  if (!settings.folder) return 'Open a folder first: a view is shared with the folder Explorer shows.';
  void atTab(key, async () => {
    const view = sharedView(name);
    if (typeof view === 'string') return note(view);
    const file = viewFile(name);
    try {
      await invoke('share_view', { folder: settings.folder, file, view });
      note(`Shared as .tlmbench/views/${file}. Anyone who opens this folder sees it under "Shared in this folder".`);
      const t = frontTab();
      if (t?.shared === file) {
        wsBase.set(t.key, wsSnapshot());
        t.dirty = false;
      }
      void loadShared();
    } catch (e) {
      note(`Couldn't share "${name}": ${e}`, () => exportWs(key, name), 'Export file…');
    }
  });
  return null;
}

/** Export file…: tab `key`'s layout (with its file and time range) as a .tlmview.json where the person chooses. */
function exportWs(key: number, name?: string): void {
  void atTab(key, async () => {
    const t = frontTab();
    const n = name ?? t?.ws ?? nameOf(t?.path ?? 'view').replace(/\.[^.]*$/, '');
    const view = sharedView(n);
    const ws = typeof view === 'string' ? toWorkspace(n, stack.panes, channelNames()) : view;
    const path = await saveDialog({ defaultPath: viewFile(n), filters: [{ name: 'TelemetryBench view', extensions: ['json'] }] });
    if (!path) return;
    try {
      await invoke('export_view', { path, view: ws });
      note(`Exported "${n}" to ${nameOf(path)}.`);
    } catch (e) {
      note(`Couldn't export "${n}": ${e}`);
    }
  });
}

/** A new, empty layout, opened in a tab. */
function newWs(raw: string): string | null {
  const name = raw.trim();
  const err = nameError(name);
  if (err) return err;
  storeWs({ schema: 1, name, panes: [{ channels: [] }] });
  void openWs(name);
  return null;
}

let progressRefresh = 0;
function onProgress(file: number, p: Progress): void {
  if (p.error) {
    const t = tabs.list.find((t) => fileOf(t.key)?.id === file);
    note(`Couldn't read ${t ? nameOf(t.path) : 'a file'} to the end: ${p.error}`);
    return;
  }
  const done = p.done >= p.total;
  const building = done ? null : { done: p.done, total: p.total, until: p.builtUntil != null ? BigInt(p.builtUntil) : null };
  // tabs behind keep their file's progress for when they come to the front
  for (const t of tabs.list) {
    const d = tabData.get(t.key);
    if (t.key !== tabs.active && d?.file.id === file) d.building = building;
  }
  if (state.file?.id !== file) return;
  state.building = building;
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

/** Lists folder `path` in Explorer; a different folder brings its own session (the current one is kept for later). */
async function showFolder(path: string): Promise<void> {
  if (settings.folder !== path && !sessionBusy) return switchSession(path, () => listFolder(path));
  await listFolder(path);
}

async function listFolder(path: string): Promise<void> {
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

/** Closes every tab and its file: the panes empty, the folder stays listed. */
async function closeAll(): Promise<void> {
  const files = [...new Set(tabs.list.filter((t) => !t.ws && !t.live).map((t) => t.path))];
  const front = frontTab()?.path;
  for (const t of [...tabs.list]) await closeTab(t.key);
  if (files.length)
    note(`Closed ${files.length === 1 ? nameOf(files[0]) : `${files.length} files`}.`, () =>
      void (async () => {
        for (const f of files) await openPath(f);
        if (front) actions.showTab(tabs.list.find((t) => t.path === front && !t.ws)?.key ?? tabs.active);
      })(),
    );
}

/** No tab: the window as at launch. */
function emptyWindow(): void {
  closePlay();
  freeLayers();
  Object.assign(state, { file: null, building: null, A: null, B: null, history: [], future: [] });
  stack.panes = [];
  Object.assign(ui, { file: '', path: '', channels: [], range: '', span: '', pointer: '', canBack: false, cursors: false, anyCursor: false, perf: '' });
  $('empty').hidden = false;
  renderPanes();
  renderOverviewHead();
  void refreshMeasure();
}

async function chooseFile(): Promise<void> {
  const path = await openDialog({ multiple: false, filters: [{ name: 'Recordings', extensions: ['csv', 'tsv', 'tlm', 'parquet'] }] });
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
    if (r.w <= 0) continue;
    const inX = x >= r.x - PAD.left && x <= r.x + r.w + rightPad();
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
  if (settings.locked && !settings.lockCursors) return void locked();
  state[which] = t;
  scheduleDraw();
  void refreshMeasure();
  if (which === 'A' && state.hoverT == null) void requestResting();
}

type Drag =
  | { type: 'pan'; cx: number; t0: bigint; t1: bigint; w: number }
  | { type: 'yaxis'; i: number; key: AxisKey; r: Rect; y: number; cy: number; from: YAxis }
  | { type: 'box'; pane: number; r: Rect }
  | { type: 'cursor'; which: 'A' | 'B'; r: Rect }
  | { type: 'overview'; r: Rect; offset: bigint }
  | { type: 'ovbox'; r: Rect; t: bigint }
  | { type: 'ovcursor'; which: 'A' | 'B'; r: Rect };
let drag: Drag | null = null;
let lastPointer: { clientX: number; clientY: number } | null = null;

// ---------- y zoom: Shift + scroll over a chart, or a drag on a y-axis; it sets the axis's fixed range ----------

/** The y-axis under the pointer: the left gutter is the first unit's, the right one the second's (null: the plot). */
function axisAt(hit: { i: number; r: Rect; x: number }): AxisKey | null {
  if (hit.x < hit.r.x) return 'u0';
  if (hit.x > hit.r.x + hit.r.w) return units(hit.i).length > 1 ? 'u1' : null;
  return null;
}

/** Zooms axis `key` of pane `i` by `factor` (under 1: in) about the value at `y` (CSS pixels on the canvases). */
function zoomAxis(i: number, key: AxisKey, r: Rect, y: number, factor: number, from: YAxis | undefined = paneY[i]?.[key]): void {
  if (!from || !(from.max > from.min) || r.h <= 0) return;
  const v = from.max - (Math.min(r.h, Math.max(0, y - r.y)) / r.h) * (from.max - from.min);
  const [lo, hi] = [v - (v - from.min) * factor, v + (from.max - v) * factor];
  if (!(hi > lo)) return;
  const real = (u: number) => (from.log ? 10 ** u : u);
  setAxis(i, key, { range: 'fixed', min: +real(lo).toPrecision(6), max: +real(hi).toPrecision(6) });
}

stackEl.addEventListener('pointerdown', (e) => {
  if (!state.file) return;
  const pane = (e.target as HTMLElement).closest<HTMLElement>('[data-pane]');
  if (pane) selectPane(Number(pane.dataset.pane));
  if (!(e.target as HTMLElement).closest('.plot')) return;
  const hit = paneAt(e);
  if (!hit) return;
  const key = e.button === 0 ? axisAt(hit) : null;
  if (key && paneY[hit.i]?.[key]) {
    e.preventDefault();
    stackEl.setPointerCapture(e.pointerId);
    drag = { type: 'yaxis', i: hit.i, key, r: hit.r, y: e.clientY - center.getBoundingClientRect().top, cy: e.clientY, from: { ...paneY[hit.i][key] } };
    return;
  }
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
  } else if (!locked()) {
    drag = { type: 'pan', cx: e.clientX, t0: state.t0, t1: state.t1, w: hit.r.w };
    center.classList.add('dragging');
  }
});

const abs = (v: bigint) => (v < 0n ? -v : v);

stackEl.addEventListener('pointermove', (e) => {
  lastPointer = e;
  const hit = paneAt(e);
  if (drag?.type === 'yaxis') {
    // up zooms in, down out, about where the drag started
    zoomAxis(drag.i, drag.key, drag.r, drag.y, Math.exp((e.clientY - drag.cy) * 0.01), drag.from);
  } else if (drag?.type === 'pan') {
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
    else if (b - a > 0 && lastPointer) flash('Drag wider to zoom', lastPointer.clientX, lastPointer.clientY, 'box-narrow');
    state.box = null;
    scheduleDraw();
  }
  drag = null;
  center.classList.remove('dragging');
};
stackEl.addEventListener('pointerup', endDrag);
stackEl.addEventListener('pointercancel', endDrag);
stackEl.addEventListener('dblclick', (e) => {
  // by position: a press on a y-axis captures the pointer, so the event lands on the stack
  const hit = paneAt(e);
  const key = hit && axisAt(hit);
  // on a y-axis: back to fitting what's in view
  if (hit && key) setAxis(hit.i, key, { range: undefined, min: undefined, max: undefined });
  else if (settings.dblFit && (e.target as HTMLElement).closest('.plot')) fit();
});

stackEl.addEventListener(
  'wheel',
  (e) => {
    const hit = paneAt(e);
    if (!state.file || !hit) return;
    e.preventDefault();
    // Shift + scroll zooms y (the time view's lock doesn't hold it; locking the layout does): the axis under the pointer, or every axis of the pane over the plot
    if (e.shiftKey) {
      let d = e.deltaY || e.deltaX;
      if (e.deltaMode === 1) d *= 16;
      const factor = Math.exp(Math.max(-200, Math.min(200, d)) * 0.0015);
      const y = e.clientY - center.getBoundingClientRect().top;
      const key = axisAt(hit);
      for (const k of key ? [key] : (Object.keys(paneY[hit.i] ?? {}) as AxisKey[])) zoomAxis(hit.i, k, hit.r, y, factor);
      return;
    }
    if (locked()) return;
    stopGlide();
    const span = Number(state.t1 - state.t0);
    let dy = e.deltaY;
    if (!settings.wheelZoom && !(e.ctrlKey || e.metaKey)) {
      // the wheel pans, a notch a tenth of the view
      if (e.deltaMode === 1) dy *= 16;
      const shift = BigInt(Math.round(Math.sign(dy) * Math.min(1, Math.abs(dy) / 100) * span * 0.1));
      setView(state.t0 + shift, state.t1 + shift);
      return;
    }
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

// overview: by the mouse mode. Box drags out a new view (a click centres on it); Pan drags the window (a click
// moves it there); Cursors places A and B; the wheel zooms
const ovTime = (clientX: number, r: Rect) => {
  const f = state.file!;
  const c = center.getBoundingClientRect();
  const t = f.first + BigInt(Math.round(((clientX - c.left - r.x) / r.w) * Number(f.last + 1n - f.first)));
  return t < f.first ? f.first : t > f.last ? f.last : t;
};
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
  const t = ovTime(e.clientX, r);
  const x = e.clientX - c.left;
  if (settings.mode === 'cursor') {
    const which = state.A == null ? 'A' : state.B == null ? 'B' : abs(t - state.A) < abs(t - state.B) ? 'A' : 'B';
    placeCursor(which, t);
    drag = { type: 'ovcursor', which, r };
    center.setPointerCapture(e.pointerId);
    return;
  }
  if (locked()) return;
  stopGlide();
  if (settings.mode === 'box') {
    state.ovBox = { x0: x, x1: x };
    drag = { type: 'ovbox', r, t };
    center.setPointerCapture(e.pointerId);
    return;
  }
  if (t < state.t0 || t > state.t1) {
    const half = (state.t1 - state.t0) / 2n;
    setView(t - half, t + half, true);
  }
  drag = { type: 'overview', r, offset: t - state.t0 };
  center.setPointerCapture(e.pointerId);
});
center.addEventListener('pointermove', (e) => {
  const f = state.file;
  if (!f || !drag) return;
  if (drag.type === 'overview') {
    const t = ovTime(e.clientX, drag.r);
    const span = state.t1 - state.t0;
    setView(t - drag.offset, t - drag.offset + span);
  } else if (drag.type === 'ovbox' && state.ovBox) {
    const c = center.getBoundingClientRect();
    state.ovBox.x1 = Math.max(drag.r.x, Math.min(drag.r.x + drag.r.w, e.clientX - c.left));
    scheduleDraw();
  } else if (drag.type === 'ovcursor') placeCursor(drag.which, ovTime(e.clientX, drag.r));
});
center.addEventListener('pointerup', (e) => {
  if (drag?.type === 'ovbox' && state.file && state.ovBox) {
    const { x0, x1 } = state.ovBox;
    const r = drag.r;
    state.ovBox = null;
    if (Math.abs(x1 - x0) >= 4) {
      const c = center.getBoundingClientRect();
      glideTo(ovTime(Math.min(x0, x1) + c.left, r), ovTime(Math.max(x0, x1) + c.left, r));
    } else {
      const half = (state.t1 - state.t0) / 2n;
      const t = ovTime(e.clientX, r);
      glideTo(t - half, t + half);
    }
    scheduleDraw();
  }
  if (drag?.type === 'overview' || drag?.type === 'ovbox' || drag?.type === 'ovcursor') drag = null;
});
center.addEventListener(
  'wheel',
  (e) => {
    const f = state.file;
    if (!f || !(e.target as HTMLElement).closest('.ovplot')) return;
    e.preventDefault();
    if (locked()) return;
    stopGlide();
    const r = ovRect()!;
    const t = ovTime(e.clientX, r);
    const factor = Math.exp(Math.max(-200, Math.min(200, e.deltaY)) * 0.0015);
    const span = Number(state.t1 - state.t0);
    const left = Number(t - state.t0);
    const t0 = t - BigInt(Math.round(left * factor));
    setView(t0, t0 + BigInt(Math.max(1, Math.round(span * factor))));
  },
  { passive: false },
);
center.addEventListener('change', (e) => {
  const t = e.target as HTMLSelectElement;
  if (!t.matches('[data-ovch]')) return;
  setSetting('overviewCh', t.value === '' ? null : Number(t.value));
  state.ovStale = true;
  renderOverviewHead();
  void requestView();
});

$('addPane').addEventListener('click', () => addPaneBelow(stack.panes.length - 1));
onStackChange(() => void layoutChanged());

// Measure's stats channel picker, wherever its panel sits
document.addEventListener('change', (e) => {
  const t = e.target as HTMLSelectElement;
  if (!t.matches('#mpStats [data-mch]')) return;
  measureCh = Number(t.value);
  void refreshMeasure();
});

// ---------- channel menu and gap factor ----------

let popup: HTMLElement | null = null;
/** A channel's menu (the Menu component), open instead of `popup`. */
let popMenu: ReturnType<typeof mount> | null = null;
function closePopup(): boolean {
  if (popMenu) {
    void unmount(popMenu);
    popMenu = null;
    return true;
  }
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
  closePopup();
  const name = f.info.channels[ch].name;
  const items: MenuItem[] = [
    { heading: name },
    { label: 'Move to a category…', meta: groupChannels(f.info.path, f.info.channels).find((g) => g.chs.includes(ch))?.name ?? '', data: { cat: '' }, run: () => categoryDialog(ch, x, y) },
    { label: 'Gap factor…', meta: `${gapFactor(ch)}×`, data: { gap: '' }, run: () => gapDialog(ch, x, y) },
  ];
  // its pick opens a dialog in `popup`: unmount the menu first (onclose), then run
  popMenu = mount(Menu, { target: document.body, props: { class: 'chmenu', label: name, at: { x, y }, items, onclose: () => closePopup() } });
}

/** Moves channel `ch` to a category of your own (a name; empty puts it back where its name says). */
function categoryDialog(ch: number, x: number, y: number): void {
  const f = state.file;
  if (!f) return;
  const c = f.info.channels[ch];
  const key = overrideKey(f.info.path, c.name);
  const cats = groupChannels(f.info.path, f.info.channels).map((g) => g.name);
  const el = document.createElement('div');
  el.className = 'dlg';
  el.setAttribute('role', 'dialog');
  el.setAttribute('aria-label', `Category of ${c.name}`);
  el.innerHTML =
    `<header><b>Category</b><span class="dsub">${esc(c.name)}</span></header>` +
    `<div class="dbody"><p>Pick one or type a new name. It's kept for this file.</p>` +
    `<label class="dfield"><input class="num" type="text" list="catlist" data-cat-input value="${esc(settings.catOverride[key] ?? '')}" placeholder="a category" style="width:180px"><datalist id="catlist">${cats.map((n) => `<option value="${esc(n)}">`).join('')}</datalist></label></div>` +
    `<footer><button class="linkbtn" data-reset>Use the default</button><span class="spacer"></span><button class="btn" data-cancel>Cancel</button><button class="btn primary" data-apply>Move</button></footer>`;
  const input = el.querySelector('input')!;
  const apply = (name: string) => {
    const next = { ...settings.catOverride };
    if (name.trim()) next[key] = name.trim();
    else delete next[key];
    setSetting('catOverride', next);
    closePopup();
  };
  el.querySelector('[data-apply]')!.addEventListener('click', () => apply(input.value));
  el.querySelector('[data-reset]')!.addEventListener('click', () => apply(''));
  el.querySelector('[data-cancel]')!.addEventListener('click', () => closePopup());
  input.addEventListener('keydown', (e) => e.key === 'Enter' && apply(input.value));
  place(el, x, y);
  input.focus();
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
  if (e.key === 'Escape' && ui.start) return void (ui.start = false);
  if (e.key === ' ' && state.file && !(e.target as HTMLElement).closest('input, select, textarea, button')) {
    e.preventDefault();
    togglePlay();
    return;
  }
  if (popup) return;
  if ((e.metaKey || e.ctrlKey) && e.key === '.') return toggleFocus();
  if ((e.metaKey || e.ctrlKey) && k === 'd' && state.file) {
    e.preventDefault();
    return duplicate(stack.sel, e.shiftKey);
  }
  if ((e.metaKey || e.ctrlKey) && k === 'n' && e.shiftKey && state.file) {
    e.preventDefault();
    return addPaneBelow(stack.sel);
  }
  if (e.metaKey || e.ctrlKey) return;
  if (e.key === 'Escape' && ui.focus) return toggleFocus(false);
  if (!(e.target as HTMLElement).closest('input, select, textarea')) {
    if (k === 't') return toggleTheme();
    if (k === 'l') return setSetting('locked', !settings.locked);
  }
  if ((e.altKey && e.key === 'ArrowLeft') || (e.key === 'Backspace' && !(e.target as HTMLElement).closest('input, textarea, select'))) return back();
  if (e.altKey && e.key === 'ArrowRight') return forward();
  if (e.key === 'Escape') return clearCursors();
  if (!state.file) return;
  // A and B over a table row place the cursors at that row's time
  const row = (k === 'a' || k === 'b') && state.file ? document.querySelector<HTMLElement>('[data-row][data-t]:hover') : null;
  if (row) return placeCursor(k === 'a' ? 'A' : 'B', BigInt(row.dataset.t!));
  // A and B over a plot place the cursors there; elsewhere B is Box zoom
  const hit = lastPointer && paneAt(lastPointer);
  if ((k === 'a' || k === 'b') && hit && state.hoverT != null) return placeCursor(k === 'a' ? 'A' : 'B', state.hoverT);
  if (k === 'b') setMode('box');
  else if (k === 'p') setMode('pan');
  else if (k === 'c') setMode('cursor');
  else if (k === 'f') fit();
  else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') panBy(e.key === 'ArrowLeft' ? -0.15 : 0.15);
  else if (k === '+' || k === '=' || e.key === 'ArrowUp') zoomBy(0.7);
  else if (k === '-' || e.key === 'ArrowDown') zoomBy(1 / 0.7);
});

/** Keyboard pan by a fraction of the view, and zoom about its centre. */
function panBy(frac: number): void {
  if (locked()) return;
  stopGlide();
  const s = BigInt(Math.round(Number(state.t1 - state.t0) * frac));
  setView(state.t0 + s, state.t1 + s);
}
function zoomBy(factor: number): void {
  if (locked()) return;
  const span = Number(state.t1 - state.t0);
  const c = Number(state.t0 + state.t1) / 2;
  const half = Math.max(1, (span * factor) / 2);
  glideTo(BigInt(Math.round(c - half)), BigInt(Math.round(c + half)));
}

function toggleTheme(): void {
  setSetting('theme', shownTheme() === 'dark' ? 'light' : 'dark');
}
function toggleFocus(on = !ui.focus): void {
  ui.focus = on;
  requestAnimationFrame(() => window.dispatchEvent(new Event('resize')));
}

Object.assign(actions, {
  redraw: () => scheduleDraw(),
  refreshViews: () => {
    state.ovStale = true;
    void requestView('exact');
    void refreshMeasure();
  },
  open: () => void chooseFile(),
  openFolder: () => void chooseFolder(),
  openPath: (p: string) => void openPath(p),
  openOnly: (p: string) =>
    void (async () => {
      await openPath(p);
      for (const t of [...tabs.list].filter((t) => t.path !== p)) await closeTab(t.key);
    })(),
  closeFile: () => void closeAll(),
  showTab: (key: number) => void showTab(key),
  closeTab: (key: number) =>
    void (async () => {
      const t = tabs.list.find((x) => x.key === key);
      await closeTab(key);
      if (t && !t.ws && !t.live) note(`Closed ${t.name}.`, () => void openPath(t.path));
    })(),
  closePath: (path: string) =>
    void (async () => {
      for (const t of tabs.list.filter((t) => t.path === path)) await closeTab(t.key);
    })(),
  resetSession: () =>
    void (async () => {
      sessionBusy = true;
      try {
        for (const t of tabs.list.filter((t) => !t.live)) await closeTab(t.key);
      } finally {
        sessionBusy = false;
      }
      const { [settings.folder]: _, ...rest } = settings.sessions;
      setSetting('sessions', rest);
    })(),
  openSession: (folder: string) => {
    if (folder === settings.folder) return;
    if (folder) void showFolder(folder);
    else
      void switchSession('', async () => {
        setSetting('folder', '');
        ui.folder = null;
      });
  },
  openPaths: (paths: string[]) =>
    void (async () => {
      for (const p of paths) await openPath(p);
    })(),
  closePaths: (paths: string[]) =>
    void (async () => {
      const gone = paths.filter((p) => tabs.list.some((t) => t.path === p));
      for (const t of tabs.list.filter((t) => gone.includes(t.path))) await closeTab(t.key);
      if (gone.length) note(`Closed ${gone.length === 1 ? nameOf(gone[0]) : `${gone.length} files`}.`, () => void (async () => { for (const p of gone) await openPath(p); })());
    })(),
  closeOtherTabs: (key: number) =>
    void (async () => {
      const gone = tabs.list.filter((t) => t.key !== key);
      await Promise.all(gone.map((t) => closeTab(t.key)));
      const files = [...new Set(gone.filter((t) => !t.ws && !t.live).map((t) => t.path))];
      if (files.length) note(`Closed ${files.length === 1 ? nameOf(files[0]) : `${files.length} files`}.`, () => void Promise.all(files.map((f) => openPath(f))));
    })(),
  saveSession,
  pickFile: async (ext: string[]) => {
    const p = await openDialog({ multiple: false, filters: [{ name: ext.join(', '), extensions: ext }] });
    return typeof p === 'string' ? p : null;
  },
  fit,
  toggleTheme,
  toggleFocus,
  back,
  clearCursors,
  refreshMeasure: () => void refreshMeasure(),
  placeCursor,
  saveAs,
  saveWs,
  revertWs,
  deleteWs,
  openWs: (name: string) => void openWs(name),
  newWs,
  loadShared: () => void loadShared().catch((e) => note(`Couldn't read the folder's shared views: ${e}`)),
  openShared: (file: string) => void openShared(file),
  share,
  exportWs: (key: number) => exportWs(key),
  axisRange: (i: number, key: AxisKey): [number, number] | null => {
    const y = paneY[i]?.[key];
    if (!y) return null;
    return y.log ? [10 ** y.min, 10 ** y.max] : [y.min, y.max];
  },
  centreOn: (t: bigint) => {
    stopGlide();
    const span = state.t1 - state.t0;
    setView(t - span / 2n, t - span / 2n + span, true);
  },
});
/** Points the time labels at the settings and the open file's start. */
function applyTimeFormat(): void {
  setTimeFormat(
    settings.timeFormat,
    settings.timeDecimals === 'auto' ? null : Number(settings.timeDecimals),
    () => state.file?.first ?? 0n,
    () => (state.file ? fileTime(state.file.info.time.kind === 'index', state.file.first) : 'relative'),
    () => (state.file ? Number(state.file.last - state.file.first) : 0),
  );
}
applyTimeFormat();

onSetting((key) => {
  if (key === 'timeFormat' || key === 'timeDecimals' || key === 'timeAxis') {
    applyTimeFormat();
    updateRange();
    void refreshMeasure();
    renderPanes();
    scheduleResize();
    return;
  }
  if (key === 'theme') {
    applyTheme();
    tokenCache = null;
    renderPanes();
  } else if (key === 'drawRaw' || key === 'gapLines') {
    if (key === 'gapLines') state.ovStale = true;
    void requestView();
  } else if (key === 'overviewTop') {
    renderOverviewHead();
    scheduleResize();
  }
});
new ResizeObserver(scheduleResize).observe(center);
// panes scroll in the stack when they don't fit: they move under the canvases, so draw again
new ResizeObserver(scheduleResize).observe(stackEl);
stackEl.addEventListener('scroll', scheduleDraw, { passive: true });

void getCurrentWebview().onDragDropEvent((e) => {
  const p = e.payload;
  center.classList.toggle('drop', p.type === 'over' || p.type === 'enter');
  if (p.type === 'drop' && p.paths.length) void openPath(p.paths[0]);
});

// tooltips: `data-tip` (CLAUDE.md); the native title for now
bindTips();
applyTheme();
updateRange();
void refreshMeasure();

/** What the UI tests drive in Play (seconds from the file's first sample). */
(window as unknown as { __tlmPlay: unknown }).__tlmPlay = {
  state: () => ({ ...play }),
  seek: (t: number) => seekPlay(t),
  speed: (s: number) => (play.speed = s),
  cursors: (a: number | null, b: number | null) => {
    const f = state.file;
    if (!f) return;
    state.A = a == null ? null : f.first + BigInt(Math.round(a * 1e9));
    state.B = b == null ? null : f.first + BigInt(Math.round(b * 1e9));
    scheduleDraw();
    void refreshMeasure();
  },
};

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
  connect: (endpoint: string, descriptor: string) => openLive(endpoint, descriptor),
  liveWindow: (s: number) => liveActions.setWindow(s),
  liveHealth: () => live.health as LiveHealth | null,
  stopLive: async () => {
    const f = state.file;
    return f ? invoke<LiveHealth>('live_stop', { file: f.id }) : null;
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
  modes: () => stack.panes.map((p) => p.mode ?? 'chart'),
  /** Each pane's axes as last drawn (log axes in log10 units). */
  axes: () => paneY.map((r) => r && JSON.parse(JSON.stringify(r))),
  sel: () => stack.sel,
  mode: () => settings.mode,
  drawn: () => state.layers.map((l) => (l ? [...l.traces.keys()] : null)),
  /** Per pane and channel: the connected stretches drawn dashed, and the solid runs. */
  dashes: () => state.layers.map((l) => (l ? [...l.traces.values()].map((t) => [t.gl.dashes.length, t.gl.runs.length]) : null)),
  /** The marks the charts draw: matches and rows left out. */
  marks: () => (ui.marks ? { pane: ui.marks.pane, hits: ui.marks.hits.length, excluded: ui.marks.excluded.length } : null),
  /** How many sample markers each pane drew last. */
  markers: () => [...markers],
  /** The overview's channels as last drawn: columns with values and those with a band. */
  overview: () => ovDrawn.map((d) => ({ ...d })),
};

if (settings.folder) void showFolder(settings.folder);
const launch = await invoke<{ file: string | null; gate: '1' | '2' | '3' | null; gateMode: string | null; gateSeconds: number | null }>('launch');
if (launch.gate === '3' && launch.file) await runGate3(gateApi, launch.file, launch.gateMode ?? '', launch.gateSeconds ?? 60);
else if (launch.gate === '2' && launch.file) await runGate2(gateApi, launch.file, launch.gateMode === 'first' ? 'first' : 'reopen');
else if (launch.gate === '1' && launch.file) {
  await openPath(launch.file);
  await runGate(gateApi);
} else {
  // the folder's session from when the app last closed, then a file it was asked to open (in front)
  await restoreSession(settings.sessions[settings.folder], launch.file);
  ui.note = '';
  if (settings.showStart && !launch.file && tabs.list.length) ui.start = true;
}
