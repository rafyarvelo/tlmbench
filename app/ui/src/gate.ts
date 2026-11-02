// The Phase 1 gate: "100M points at 60 fps". A scripted 30-second sweep over the open file, driven one view
// per animation frame (zoom from the whole file to 1 s and back, and pans), then the IPC cost alone. The report
// goes to stdout and the app exits 0 (pass) or 1.
//
// WebKit rounds the page's clocks to 1 ms, so frames are judged by missed vsyncs rather than percentiles of
// 16.7 ms: a frame is missed when its interval exceeds 1.5 refresh periods.
//
// Pass:
// - at most 1% of frames missed, and no interval over 3 refresh periods;
// - the core reduces a view with at most 1M samples in p95 < 5 ms (the design's "viewport query in Rust < 5 ms per
//   trace"), timed in Rust. Views over more samples (whole-file views of 100M raw samples) are reported, not gated:
//   the pyramid (Phase 2) bounds them.
// A run during which the window was hidden (display asleep, another app in front) is reported as invalid.

import { invoke } from '@tauri-apps/api/core';

import type { QueryStats, RecordingInfo } from './main';

export interface GateApi {
  readonly file: null | { info: RecordingInfo; first: bigint; last: bigint };
  widthPx(): number;
  setView(t0: bigint, t1: bigint): void;
  draw(): void;
  onQuery: ((q: QueryStats) => void) | null;
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

export async function runGate(api: GateApi): Promise<void> {
  const f = api.file;
  if (!f) {
    await invoke('gate_report', { report: { pass: false, error: 'the gate file did not open' } });
    return;
  }
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

  const echo: number[] = [];
  for (let i = 0; i < 200; i++) {
    const t = performance.now();
    await invoke<ArrayBuffer>('echo', { bytes: 65_536 });
    echo.push(performance.now() - t);
  }

  const frameStats = stats(intervals);
  const period = frameStats.p50;
  const missed = intervals.filter((i) => i > 1.5 * period).length;
  const longest = frameStats.max;
  const zoomed = queries.filter((q) => q.samplesInView <= 1_000_000);
  const whole = queries.filter((q) => q.samplesInView >= f.info.samples / 2);
  const reduceZoomed = stats(zoomed.map((q) => q.reduceUs / 1000));
  const checks = {
    missed_frames_le_1pct: missed <= intervals.length / 100,
    no_interval_over_3_periods: longest <= 3 * period,
    zoomed_reduce_p95_ms_lt_5: reduceZoomed.n > 0 && reduceZoomed.p95 < 5,
  };
  const valid = !hidden;
  const report = {
    gate: 'Phase 1: 100M points at 60 fps',
    valid,
    note: valid ? undefined : 'the window was hidden during the run (display asleep or covered): run it again',
    recording: f.info.path,
    samples: f.info.samples,
    width_px: api.widthPx(),
    duration_s: DURATION_MS / 1000,
    refresh_hz: Math.round(1000 / period),
    frames: intervals.length,
    missed_frames: missed,
    frame_interval_ms: frameStats,
    draw_ms: stats(drawMs),
    reduce_zoomed_ms: reduceZoomed,
    reduce_all_ms: stats(queries.map((q) => q.reduceUs / 1000)),
    reduce_whole_file_ms: stats(whole.map((q) => q.reduceUs / 1000)),
    round_trip_ms: stats(queries.map((q) => q.ms)),
    points_per_reply: stats(queries.map((q) => q.points)),
    ipc_echo_64k_ms: stats(echo),
    checks,
    pass: valid && Object.values(checks).every(Boolean),
  };
  await invoke('gate_report', { report });
}
