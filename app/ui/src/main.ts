// Phase 1 UI: one pane. Pan and zoom re-project the last reply at once (two uniforms), while the core computes
// the sharper one; replies carry a generation and older ones are dropped. At most one query is in flight, and
// the latest view is asked for as soon as it returns.

import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

import { drawAxes, PAD } from './axes';
import { runGate } from './gate';
import { type Axis, type Rect, TraceRenderer } from './gl';
import { decode, type Reply } from './wire';

interface ChannelInfo {
  name: string;
  unit: string | null;
  dtype: string;
}

export interface RecordingInfo {
  path: string;
  format: string;
  samples: number;
  channels: ChannelInfo[];
  skipped: string[];
}

interface Opened {
  id: number;
  info: RecordingInfo;
  /** First and last sample times as decimal strings (exact i64 nanoseconds). */
  range: [string, string] | null;
}

export interface QueryStats {
  /** Round trip, as the page's clock sees it (1 ms resolution in WebKit). */
  ms: number;
  /** The core's reduce time, measured in Rust. */
  reduceUs: number;
  points: number;
  bytes: number;
  samplesInView: number;
}

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const pane = $<HTMLDivElement>('pane');
const glCanvas = $<HTMLCanvasElement>('gl');
const axesCanvas = $<HTMLCanvasElement>('axes');
const statusEl = $<HTMLSpanElement>('status');
const fileEl = $<HTMLSpanElement>('file');
const channelEl = $<HTMLSelectElement>('channel');
const emptyEl = $<HTMLDivElement>('empty');

const renderer = new TraceRenderer(glCanvas);

const state = {
  file: null as null | { id: number; info: RecordingInfo; first: bigint; last: bigint },
  channel: 0,
  t0: 0n,
  t1: 1n,
  reply: null as Reply | null,
  yMin: -1,
  yMax: 1,
  generation: 0,
  drawnGeneration: 0,
  inflight: false,
  pending: false,
  drawScheduled: false,
  onQuery: null as null | ((q: QueryStats) => void),
};

function plotRect(): Rect {
  return {
    x: PAD.left,
    y: PAD.top,
    w: Math.max(1, pane.clientWidth - PAD.left - PAD.right),
    h: Math.max(1, pane.clientHeight - PAD.top - PAD.bottom),
  };
}

/** Pixel columns the core reduces for: the plot width in device pixels. */
function widthPx(): number {
  return Math.max(1, Math.round(plotRect().w * (window.devicePixelRatio || 1)));
}

function samplesInView(): number {
  const f = state.file;
  if (!f || f.last <= f.first) return f?.info.samples ?? 0;
  const lo = state.t0 > f.first ? state.t0 : f.first;
  const hi = state.t1 < f.last ? state.t1 : f.last;
  return hi <= lo ? 0 : Math.round((f.info.samples * Number(hi - lo)) / Number(f.last - f.first));
}

async function requestView(): Promise<void> {
  const f = state.file;
  if (!f) return;
  if (state.inflight) {
    state.pending = true;
    return;
  }
  state.inflight = true;
  state.pending = false;
  const generation = ++state.generation;
  const inView = samplesInView();
  const started = performance.now();
  try {
    const buf = await invoke<ArrayBuffer>('view', {
      file: f.id,
      channels: [state.channel],
      t0: state.t0.toString(),
      t1: state.t1.toString(),
      widthPx: widthPx(),
      generation,
    });
    const reply = decode(buf);
    const ms = performance.now() - started;
    const trace = reply.traces[0];
    if (generation > state.drawnGeneration && trace) {
      state.drawnGeneration = generation;
      state.reply = reply;
      renderer.setTrace(trace.x, trace.y);
      if (Number.isFinite(trace.yMin) && Number.isFinite(trace.yMax)) {
        const pad = (trace.yMax - trace.yMin) * 0.05 || Math.abs(trace.yMax) * 0.05 || 1;
        state.yMin = trace.yMin - pad;
        state.yMax = trace.yMax + pad;
      }
      const kind = trace.kind === 'm4' ? 'M4' : 'raw';
      statusEl.textContent = `${trace.x.length.toLocaleString()} points · ${kind} · ${ms.toFixed(1)} ms`;
      state.onQuery?.({ ms, reduceUs: trace.reduceUs, points: trace.x.length, bytes: reply.bytes, samplesInView: inView });
      scheduleDraw();
    }
  } catch (e) {
    statusEl.textContent = String(e);
  } finally {
    state.inflight = false;
    if (state.pending) void requestView();
  }
}

function draw(): void {
  state.drawScheduled = false;
  renderer.resize();
  const rect = plotRect();
  const span = Number(state.t1 - state.t0);
  const r = state.reply;
  if (r && span > 0) {
    const x: Axis = { a: (2 * r.xScaleNs) / span, b: (2 * Number(r.originNs - state.t0)) / span - 1 };
    const yr = state.yMax - state.yMin || 1;
    const y: Axis = { a: 2 / yr, b: (-2 * state.yMin) / yr - 1 };
    renderer.draw(x, y, traceColor(), rect);
  } else {
    renderer.clear();
  }
  if (state.file) drawAxes(axesCanvas, { t0: state.t0, t1: state.t1, yMin: state.yMin, yMax: state.yMax });
}

function scheduleDraw(): void {
  if (state.drawScheduled) return;
  state.drawScheduled = true;
  requestAnimationFrame(draw);
}

let cachedColor: [number, number, number, number] | null = null;
function traceColor(): [number, number, number, number] {
  if (cachedColor) return cachedColor;
  const probe = document.createElement('span');
  probe.style.color = 'var(--trace)';
  document.body.append(probe);
  const m = getComputedStyle(probe).color.match(/[\d.]+/g)?.map(Number) ?? [37, 99, 235];
  probe.remove();
  cachedColor = [m[0] / 255, m[1] / 255, m[2] / 255, 1];
  return cachedColor;
}
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
  cachedColor = null;
  scheduleDraw();
});

/** Sets the view, clamped to a sensible zoom range, and asks for it. */
function setView(t0: bigint, t1: bigint): void {
  const f = state.file;
  if (!f) return;
  const full = f.last - f.first || 1n;
  let span = t1 - t0;
  const minSpan = 1000n; // 1 µs
  if (span < minSpan) {
    const mid = t0 + span / 2n;
    span = minSpan;
    t0 = mid - span / 2n;
  }
  const maxSpan = full * 3n;
  if (span > maxSpan) {
    const mid = t0 + span / 2n;
    span = maxSpan;
    t0 = mid - span / 2n;
  }
  state.t0 = t0;
  state.t1 = t0 + span;
  scheduleDraw();
  void requestView();
}

function fit(): void {
  const f = state.file;
  if (f) setView(f.first, f.last + 1n);
}

async function openPath(path: string): Promise<void> {
  statusEl.textContent = 'Opening…';
  try {
    if (state.file) await invoke('close', { file: state.file.id });
    const o = await invoke<Opened>('open', { path });
    if (!o.range) throw new Error('This file has no samples');
    state.file = { id: o.id, info: o.info, first: BigInt(o.range[0]), last: BigInt(o.range[1]) };
    state.channel = 0;
    state.reply = null;
    fileEl.textContent = path.split(/[\\/]/).pop() ?? path;
    fileEl.dataset.tip = path;
    channelEl.replaceChildren(
      ...o.info.channels.map((c, i) => new Option(c.unit ? `${c.name} (${c.unit})` : c.name, String(i))),
    );
    channelEl.hidden = o.info.channels.length < 2;
    emptyEl.hidden = true;
    fit();
  } catch (e) {
    statusEl.textContent = String(e);
  }
}

async function chooseFile(): Promise<void> {
  const path = await openDialog({
    multiple: false,
    filters: [{ name: 'Recordings', extensions: ['csv', 'tsv', 'tlm'] }],
  });
  if (typeof path === 'string') await openPath(path);
}

// Interaction: wheel or pinch zooms around the pointer, horizontal scroll and drag pan, double-click fits.
pane.addEventListener(
  'wheel',
  (e) => {
    if (!state.file) return;
    e.preventDefault();
    const rect = plotRect();
    const span = Number(state.t1 - state.t0);
    if (e.ctrlKey || Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      const factor = Math.exp(e.deltaY * (e.ctrlKey ? 0.01 : 0.002));
      const frac = Math.min(1, Math.max(0, (e.offsetX - rect.x) / rect.w));
      const anchor = frac * span; // the time under the pointer, from t0
      const t0 = state.t0 + BigInt(Math.round(anchor - anchor * factor));
      setView(t0, t0 + BigInt(Math.max(1, Math.round(span * factor))));
    } else {
      const shift = BigInt(Math.round((e.deltaX / rect.w) * span));
      setView(state.t0 + shift, state.t1 + shift);
    }
  },
  { passive: false },
);

let drag: null | { x: number; t0: bigint; t1: bigint } = null;
pane.addEventListener('pointerdown', (e) => {
  if (!state.file || e.button !== 0) return;
  drag = { x: e.clientX, t0: state.t0, t1: state.t1 };
  pane.setPointerCapture(e.pointerId);
  pane.classList.add('dragging');
});
pane.addEventListener('pointermove', (e) => {
  if (!drag) return;
  const shift = BigInt(Math.round(((drag.x - e.clientX) / plotRect().w) * Number(drag.t1 - drag.t0)));
  setView(drag.t0 + shift, drag.t1 + shift);
});
const endDrag = () => {
  drag = null;
  pane.classList.remove('dragging');
};
pane.addEventListener('pointerup', endDrag);
pane.addEventListener('pointercancel', endDrag);
pane.addEventListener('dblclick', fit);

window.addEventListener('keydown', (e) => {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'o') {
    e.preventDefault();
    void chooseFile();
  } else if (e.key === 'f' && !e.metaKey && !e.ctrlKey) {
    fit();
  }
});

$<HTMLButtonElement>('open').addEventListener('click', () => void chooseFile());
channelEl.addEventListener('change', () => {
  state.channel = Number(channelEl.value);
  state.reply = null;
  void requestView();
});
new ResizeObserver(() => {
  scheduleDraw();
  void requestView();
}).observe(pane);

void getCurrentWebview().onDragDropEvent((e) => {
  const p = e.payload;
  if (p.type === 'over' || p.type === 'enter') pane.classList.add('drop');
  else pane.classList.remove('drop');
  if (p.type === 'drop' && p.paths.length) void openPath(p.paths[0]);
});

// tooltips: `data-tip` (CLAUDE.md); the native title for now
for (const el of document.querySelectorAll<HTMLElement>('[data-tip]')) el.title = el.dataset.tip ?? '';

/** What the gate script drives. */
export const gateApi = {
  get file() {
    return state.file;
  },
  get view() {
    return { t0: state.t0, t1: state.t1 };
  },
  widthPx,
  /** Sets the view without scheduling a frame: the gate draws in its own animation frames. */
  setView(t0: bigint, t1: bigint) {
    state.t0 = t0;
    state.t1 = t1;
    void requestView();
  },
  draw,
  set onQuery(f: ((q: QueryStats) => void) | null) {
    state.onQuery = f;
  },
};

const launch = await invoke<{ file: string | null; gate: boolean }>('launch');
if (launch.file) await openPath(launch.file);
if (launch.gate) await runGate(gateApi);
