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
//
// Phase 3, "4M samples/s with no drops", the app's half (`just gate3 app`): a live tab follows a TCP source
// (`tlmbench-gen --send`) with a 10 s window until the stream ends. Gated here, as the Roadmap's decision words it:
// at most 1% of frames missed while it follows, nothing dropped by the ring buffer or missing by sequence, the ring
// at most half full; the longest frame is reported. ci/gate3.py then checks the recording record by record against
// the sender.

import { invoke } from '@tauri-apps/api/core';

import type { QueryStats, RecordingInfo } from './app';

export interface GateApi {
  readonly file: null | { id: number; info: RecordingInfo; first: bigint; last: bigint };
  readonly panes: number[][];
  widthPx(): number;
  setView(t0: bigint, t1: bigint): void;
  draw(): void;
  open(path: string): Promise<void>;
  onQuery: ((q: QueryStats) => void) | null;
  onPaint: ((p: { coarse: boolean; complete: boolean }) => void) | null;
  built(): Promise<void>;
  /** Phase 3: connects a live source in a tab of its own, following now. */
  connect(endpoint: string, descriptor: string): Promise<void>;
  liveWindow(seconds: number): void;
  /** The live source's health as last polled (null before the first poll). */
  liveHealth(): LiveHealth | null;
  stopLive(): Promise<LiveHealth | null>;
}

export interface LiveHealth {
  records_written: number;
  records_dropped: number;
  sequence_missing: number;
  sequence_repeated: number;
  ring_bytes: number;
  ring_high_water_bytes: number;
  ended: boolean;
  error: string | null;
  files: string[];
  seconds: number;
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
    longest_frame_periods: Math.round((frameStats.max / period) * 10) / 10,
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

export async function runGate3(api: GateApi, endpoint: string, descriptor: string, seconds: number): Promise<void> {
  const report: Record<string, unknown> = { gate: 'Phase 3: 4M samples/s with no drops (app)', endpoint, pass: false };
  try {
    await api.connect(endpoint, descriptor);
    if (!api.file) throw new Error(`couldn't connect to ${endpoint}`);
  } catch (e) {
    report.error = String(e);
    await invoke('gate_report', { report });
    return;
  }
  api.liveWindow(10);
  const intervals: number[] = [];
  const queries: QueryStats[] = [];
  /** Frames over 40 ms, when they came, and the query before each (reported, to see what stalled). */
  const long: { at_s: number; ms: number; last_query: QueryStats | null }[] = [];
  api.onQuery = (q) => queries.push(q);
  let hidden = document.hidden;
  const onVisibility = () => (hidden ||= document.hidden);
  document.addEventListener('visibilitychange', onVisibility);
  // the app's own follow loop draws each frame; this only times them, from a second after the tab opened (as Gate 1
  // starts after the first paint) until the stream ends
  await new Promise<void>((done) => {
    let start = -1;
    let prev = -1;
    const frame = (now: number) => {
      if (start < 0) start = now;
      if (prev >= 0 && now - start > 1000) {
        intervals.push(now - prev);
        if (now - prev > 40) long.push({ at_s: Math.round(now - start) / 1000, ms: Math.round(now - prev), last_query: queries[queries.length - 1] ?? null });
      }
      prev = now;
      if (api.liveHealth()?.ended || now - start > (seconds + 10) * 1000) return done();
      requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
  });
  api.onQuery = null;
  document.removeEventListener('visibilitychange', onVisibility);
  const health = await api.stopLive();
  const frameStats = stats(intervals);
  const period = frameStats.p50;
  const missed = intervals.filter((i) => i > 1.5 * period).length;
  const ok = {
    missed_frames_le_1pct: missed <= intervals.length / 100,
    none_dropped: health != null && health.records_dropped === 0,
    none_missing: health != null && health.sequence_missing === 0 && health.sequence_repeated === 0,
    ring_le_half: health != null && health.ring_high_water_bytes <= health.ring_bytes / 2,
    no_error: health != null && health.error == null,
  };
  Object.assign(report, {
    valid: !hidden,
    refresh_hz: Math.round(1000 / period),
    frames: intervals.length,
    missed_frames: missed,
    frame_interval_ms: frameStats,
    round_trip_ms: stats(queries.map((q) => q.ms)),
    reduce_ms: stats(queries.map((q) => q.reduceUs / 1000)),
    points_per_reply: stats(queries.map((q) => q.points)),
    long_frames: long,
    health,
    recording: health?.files ?? [],
    checks: ok,
    pass: !hidden && Object.values(ok).every(Boolean),
  });
  await invoke('gate_report', { report });
}

/** What `ci/gate4.py` asks of one file: its name, how closely it must match, and each channel's expected statistics. */
interface Gate4File {
  name: string;
  path: string;
  /** Relative tolerance (0: the numbers must match exactly). */
  tolerance: number;
  expect: Record<string, { count: number; min: number; max: number; mean: number; std: number }>;
}

/** Gate 4 in the app: every file of the format matrix opens through the app's own open path (a descriptor found by its
 * file patterns where the format needs one) with the same per-channel count, extremes, mean and standard deviation. */
export async function runGate4(api: GateApi, manifest: string): Promise<void> {
  const m = await invoke<{ files: Gate4File[] }>('gate_manifest', { path: manifest });
  const results: Record<string, string> = {};
  const close = (a: number, b: number, tol: number) => a === b || Math.abs(a - b) <= tol * Math.max(1, Math.abs(a), Math.abs(b));
  for (const f of m.files) {
    try {
      await api.open(f.path);
      const file = api.file;
      if (!file || file.info.path !== f.path) {
        results[f.name] = 'did not open';
        continue;
      }
      const names = file.info.channels.map((c) => c.name);
      const wanted = Object.keys(f.expect).filter((n) => names.includes(n));
      if (wanted.length !== Object.keys(f.expect).length) {
        results[f.name] = `channels ${names.join(',')} lack some of ${Object.keys(f.expect).join(',')}`;
        continue;
      }
      // exact statistics come from the finished pyramid: ask until the core has it (it answers nothing before)
      const ask = () => invoke<{ channel: number; count: number; min: number | null; max: number | null; mean: number | null; std: number | null }[] | null>('stats', {
        file: file.id,
        channels: wanted.map((n) => names.indexOf(n)),
        span: { t0: file.first.toString(), t1: (file.last + 1n).toString() },
      });
      let got = await ask();
      for (let k = 0; !got && k < 1200; k++) {
        await new Promise((r) => setTimeout(r, 100));
        got = await ask();
      }
      const bad: string[] = [];
      wanted.forEach((n, i) => {
        const g = got?.[i];
        const e = f.expect[n];
        if (!g) return void bad.push(`${n}: no statistics`);
        if (g.count !== e.count) bad.push(`${n}: count ${g.count} != ${e.count}`);
        for (const k of ['min', 'max', 'mean', 'std'] as const) if (g[k] == null || !close(g[k]!, e[k], k === 'mean' || k === 'std' ? Math.max(f.tolerance, 1e-9) : f.tolerance)) bad.push(`${n}: ${k} ${g[k]} != ${e[k]}`);
      });
      results[f.name] = bad.length ? bad.join('; ') : 'same';
    } catch (e) {
      results[f.name] = `error: ${e}`;
    }
  }
  const failed = Object.entries(results).filter(([, v]) => v !== 'same');
  await invoke('gate_report', { report: { gate: 'Phase 4: formats in the app', results, failures: failed.map(([k, v]) => `${k}: ${v}`), pass: failed.length === 0 && m.files.length > 0 } });
}
