// The gates, run in the release app with a scripted sweep; the report goes to stdout and the app exits 0 (pass)
// or 1. WebKit rounds the page's clocks to 1 ms, so frames are judged by missed vsyncs rather than percentiles of
// 16.7 ms: a frame is missed when its interval exceeds 1.5 refresh periods. A run during which the window was
// hidden (display asleep, another app in front) is reported as invalid.
//
// Phase 1, "100M points at 60 fps" (`just gate`): a 30-second sweep over one channel (zoom from the whole file to
// 1 s and back, and pans), then the IPC cost alone. Pass: at most 1% of frames missed and no interval over 3
// refresh periods; the core reduces a view of at most 1M samples in p95 < 5 ms, timed in Rust.
//
// Phase 2, "10 GB first paint under 1 s" (`just gate2`), in one of two modes:
// - reopen: the pyramid is cached and the file's pages were evicted. Gated: the whole view's first paint (coarse
//   views count: they hide no extreme and are labelled) within 1 s of asking to open. Reported: when the exact
//   view replaced it, and the same sweep as Phase 1 over every channel (the 20-trace target).
// - first: nothing is cached. Reported: when axes and build progress showed, and how long the build took.

import { invoke } from '@tauri-apps/api/core';

import type { QueryStats, RecordingInfo } from './main';

export interface GateApi {
  readonly file: null | { info: RecordingInfo; first: bigint; last: bigint };
  readonly panes: number[][];
  widthPx(): number;
  setView(t0: bigint, t1: bigint): void;
  draw(): void;
  open(path: string): Promise<void>;
  onQuery: ((q: QueryStats) => void) | null;
  onPaint: ((p: { coarse: boolean; complete: boolean }) => void) | null;
  built(): Promise<void>;
}

const DURATION_MS = 30_000;
const NS = 1e9;

interface Stats {
  n: number;
  mean: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
}

function stats(xs: number[]): Stats {
  const s = [...xs].sort((a, b) => a - b);
  const q = (p: number) => (s.length ? s[Math.min(s.length - 1, Math.floor(p * s.length))] : NaN);
  const round = (v: number) => Math.round(v * 1000) / 1000;
  return {
    n: s.length,
    mean: round(s.reduce((a, b) => a + b, 0) / (s.length || 1)),
    p50: round(q(0.5)),
    p95: round(q(0.95)),
    p99: round(q(0.99)),
    max: round(s.length ? s[s.length - 1] : NaN),
  };
}

/** The view at `ms` into the sweep, as [t0, t1] offsets in ns from the first sample. */
function viewAt(ms: number, full: number): [number, number] {
  const mid = full / 2;
  const small = Math.min(NS, full / 10); // 1 s window, or less for short files
  const lerpLog = (a: number, b: number, p: number) => a * (b / a) ** p;
  const around = (c: number, w: number): [number, number] => {
    const lo = Math.min(Math.max(0, c - w / 2), Math.max(0, full - w));
    return [lo, lo + w];
  };
  const p = ms / 1000;
  if (p < 10) return around(mid, lerpLog(full, small, p / 10)); // zoom in
  if (p < 15) return around(mid + (p - 10) * 12 * small, small); // pan at 1 s
  if (p < 25) return around(mid + 60 * small, lerpLog(small, full, (p - 15) / 10)); // zoom out
  return around(full * (0.1 + 0.8 * ((p - 25) / 5)), full / 10); // pan at 10%
}

/** The scripted sweep, one view per animation frame, and what it measured. */
async function sweep(api: GateApi) {
  const f = api.file!;
  const full = Number(f.last - f.first);
  const intervals: number[] = [];
  const drawMs: number[] = [];
  const queries: QueryStats[] = [];
  api.onQuery = (q) => queries.push(q);
  let hidden = document.hidden;
  const onVisibility = () => (hidden ||= document.hidden);
  document.addEventListener('visibilitychange', onVisibility);
  await new Promise<void>((done) => {
    let start = -1;
    let prev = -1;
    const frame = (now: number) => {
      if (start < 0) start = now;
      if (prev >= 0) intervals.push(now - prev);
      prev = now;
      const elapsed = now - start;
      if (elapsed >= DURATION_MS) return done();
      const [a, b] = viewAt(elapsed, full);
      api.setView(f.first + BigInt(Math.round(a)), f.first + BigInt(Math.max(1, Math.round(b))));
      const t = performance.now();
      api.draw();
      drawMs.push(performance.now() - t);
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
  });
  api.onQuery = null;
  document.removeEventListener('visibilitychange', onVisibility);
  const frameStats = stats(intervals);
  const period = frameStats.p50;
  const missed = intervals.filter((i) => i > 1.5 * period).length;
  const zoomed = queries.filter((q) => q.samplesInView <= 1_000_000);
  const whole = queries.filter((q) => q.samplesInView >= f.info.samples / 2);
  return {
    valid: !hidden,
    period,
    missed,
    intervals: intervals.length,
    longest: frameStats.max,
    reduceZoomed: stats(zoomed.map((q) => q.reduceUs / 1000)),
    report: {
      duration_s: DURATION_MS / 1000,
      refresh_hz: Math.round(1000 / period),
      frames: intervals.length,
      missed_frames: missed,
      frame_interval_ms: frameStats,
      draw_ms: stats(drawMs),
      reduce_zoomed_ms: stats(zoomed.map((q) => q.reduceUs / 1000)),
      reduce_all_ms: stats(queries.map((q) => q.reduceUs / 1000)),
      reduce_whole_file_ms: stats(whole.map((q) => q.reduceUs / 1000)),
      round_trip_ms: stats(queries.map((q) => q.ms)),
      points_per_reply: stats(queries.map((q) => q.points)),
      traces_per_reply: stats(queries.map((q) => q.traces)),
    },
  };
}

export async function runGate(api: GateApi): Promise<void> {
  const f = api.file;
  if (!f) {
    await invoke('gate_report', { report: { pass: false, error: 'the gate file did not open' } });
    return;
  }
  const s = await sweep(api);
  const echo: number[] = [];
  for (let i = 0; i < 200; i++) {
    const t = performance.now();
    await invoke<ArrayBuffer>('echo', { bytes: 65_536 });
    echo.push(performance.now() - t);
  }
  const checks = {
    missed_frames_le_1pct: s.missed <= s.intervals / 100,
    no_interval_over_3_periods: s.longest <= 3 * s.period,
    zoomed_reduce_p95_ms_lt_5: s.reduceZoomed.n > 0 && s.reduceZoomed.p95 < 5,
  };
  const report = {
    gate: 'Phase 1: 100M points at 60 fps',
    valid: s.valid,
    note: s.valid ? undefined : 'the window was hidden during the run (display asleep or covered): run it again',
    recording: f.info.path,
    samples: f.info.samples,
    width_px: api.widthPx(),
    ...s.report,
    ipc_echo_64k_ms: stats(echo),
    checks,
    pass: s.valid && Object.values(checks).every(Boolean),
  };
  await invoke('gate_report', { report });
}

export async function runGate2(api: GateApi, path: string, mode: 'reopen' | 'first'): Promise<void> {
  const marks: { axes?: number; firstPaint?: number; firstPaintCoarse?: boolean; exact?: number } = {};
  let started = performance.now();
  api.onPaint = (p) => {
    const t = performance.now() - started;
    marks.axes ??= t;
    if (p.complete && marks.firstPaint == null) {
      marks.firstPaint = t;
      marks.firstPaintCoarse = p.coarse;
    }
    if (p.complete && !p.coarse) marks.exact ??= t;
  };
  started = performance.now();
  await api.open(path);
  const f = api.file;
  if (!f) {
    await invoke('gate_report', { report: { pass: false, error: 'the gate file did not open' } });
    return;
  }
  const base = {
    gate: 'Phase 2: 10 GB first paint under 1 s',
    mode,
    recording: f.info.path,
    channels: f.info.channels.length,
    samples_per_channel: f.info.samples,
    panes: api.panes.length,
    width_px: api.widthPx(),
    since_page_load_ms: Math.round(started),
  };
  if (mode === 'first') {
    await api.built();
    const buildS = (performance.now() - started) / 1000;
    const bytes = f.info.samples * f.info.channels.length * 4;
    api.onPaint = null;
    const report = {
      ...base,
      axes_and_progress_ms: marks.axes,
      axes_and_progress_lt_1s: marks.axes != null && marks.axes < 1000,
      build_s: Math.round(buildS * 100) / 100,
      build_mb_per_s: Math.round(bytes / buildS / 1e6),
      pass: true, // reported, not gated
    };
    await invoke('gate_report', { report });
    return;
  }
  // reopen: wait for the exact view (or 20 s), then sweep every channel
  const deadline = performance.now() + 20_000;
  while (marks.exact == null && performance.now() < deadline) await new Promise((r) => setTimeout(r, 20));
  api.onPaint = null;
  const s = await sweep(api);
  const checks = { first_paint_lt_1s: marks.firstPaint != null && marks.firstPaint < 1000 };
  const report = {
    ...base,
    valid: s.valid,
    note: s.valid ? undefined : 'the window was hidden during the run (display asleep or covered): run it again',
    first_paint_ms: marks.firstPaint != null ? Math.round(marks.firstPaint) : null,
    first_paint_coarse: marks.firstPaintCoarse,
    exact_ms: marks.exact != null ? Math.round(marks.exact) : null,
    sweep_20_traces: s.report,
    checks,
    pass: s.valid && Object.values(checks).every(Boolean),
  };
  await invoke('gate_report', { report });
}
