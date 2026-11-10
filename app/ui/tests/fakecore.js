// A fake of the app's core for the UI tests: it answers the Tauri commands the UI calls (launch, open, close,
// view, values_at, stats, rows, the dialog and the drag-and-drop events) from synthetic recordings, and encodes view
// batches in the core's wire format (crates/core/src/wire.rs; t_wire checks it against the core's golden file).
//
// Loaded before the page (an init script), it installs window.__TAURI_INTERNALS__ the way Tauri's mockIPC does.
// Tests drive it through window.__tlmFake: `files` (path → recording), `launch` (a file opened at start), `pick`
// (what Open… returns; a file spec's `cold` makes exact views slow, `exactMs` of them, 800 by default), `drop(paths)`, `build(path, fraction)` (build progress), `calls` (commands seen).
// In Node, require() it for the encoder alone (globalThis.__tlmFakeCore).
(() => {
  'use strict';

  // ---------- wire format ----------

  const HEADER = 40;
  const TRACE_HEADER = 32;
  const BATCH_HEADER = 16;
  const BATCH_ENTRY = 16;
  const up8 = (n) => (n + 7) & ~7;

  const bucketNs = (vp) => {
    const b = (BigInt(vp.t1) - BigInt(vp.t0)) / BigInt(Math.max(1, vp.widthPx));
    return b < 1n ? 1n : b;
  };

  /** One reply: `traces` are { channel, kind: 'raw'|'m4', coarse, fromPyramid, reduceUs, t: bigint[], y: number[] }. */
  function encode(generation, vp, builtUntil, traces) {
    const size = HEADER + traces.reduce((a, t) => a + TRACE_HEADER + 8 * t.t.length + up8(4 * (t.dashed?.length ?? 0)), 0);
    const buf = new ArrayBuffer(size);
    const v = new DataView(buf);
    const t0 = BigInt(vp.t0);
    const t1 = BigInt(vp.t1);
    const scale = Number(bucketNs(vp));
    [...'TLMV'].forEach((c, i) => v.setUint8(i, c.charCodeAt(0)));
    v.setUint16(4, 3, true);
    v.setUint16(6, builtUntil != null ? 1 : 0, true);
    v.setUint32(8, generation, true);
    v.setUint32(12, traces.length, true);
    v.setBigInt64(16, t0, true);
    v.setFloat64(24, scale, true);
    v.setBigInt64(32, builtUntil != null ? BigInt(builtUntil) : 0x7fffffffffffffffn, true);
    let o = HEADER;
    for (const t of traces) {
      let lo = Infinity;
      let hi = -Infinity;
      t.t.forEach((ts, i) => {
        const y = Math.fround(t.y[i]);
        if (!Number.isNaN(y) && ts >= t0 && ts < t1) [lo, hi] = [Math.min(lo, y), Math.max(hi, y)];
      });
      if (lo > hi) lo = hi = NaN;
      const n = t.t.length;
      v.setUint32(o, t.channel, true);
      v.setUint32(o + 4, n, true);
      v.setUint32(o + 8, (t.kind === 'm4' ? 1 : 0) | (t.coarse ? 2 : 0) | (t.fromPyramid ? 4 : 0), true);
      v.setUint32(o + 12, t.reduceUs ?? 0, true);
      v.setFloat32(o + 16, lo, true);
      v.setFloat32(o + 20, hi, true);
      v.setUint32(o + 24, t.dashed?.length ?? 0, true);
      const d = o + TRACE_HEADER;
      t.t.forEach((ts, i) => v.setFloat32(d + 4 * i, Number(ts - t0) / scale, true));
      t.y.forEach((y, i) => v.setFloat32(d + 4 * n + 4 * i, y, true));
      (t.dashed ?? []).forEach((k, i) => v.setUint32(d + 8 * n + 4 * i, k, true));
      o += TRACE_HEADER + 8 * n + up8(4 * (t.dashed?.length ?? 0));
    }
    return buf;
  }

  /** Frames replies as a batch: `parts` are [id, ArrayBuffer]. */
  function encodeBatch(generation, parts) {
    const table = BATCH_HEADER + BATCH_ENTRY * parts.length;
    const size = parts.reduce((a, [, m]) => a + up8(m.byteLength), table);
    const out = new Uint8Array(size);
    const v = new DataView(out.buffer);
    [...'TLMB'].forEach((c, i) => v.setUint8(i, c.charCodeAt(0)));
    v.setUint16(4, 1, true);
    v.setUint32(8, generation, true);
    v.setUint32(12, parts.length, true);
    let at = table;
    parts.forEach(([id, m], k) => {
      const e = BATCH_HEADER + BATCH_ENTRY * k;
      v.setUint32(e, id, true);
      v.setUint32(e + 8, at, true);
      v.setUint32(e + 12, m.byteLength, true);
      out.set(new Uint8Array(m), at);
      at += up8(m.byteLength);
    });
    return out.buffer;
  }

  globalThis.__tlmFakeCore = { encode, encodeBatch };
  if (typeof window === 'undefined') return;

  // ---------- synthetic recordings ----------

  const T0 = 1767225600000000000n; // 2026-01-01T00:00:00Z
  /** A recording: `rate` Hz for `samples` samples; channel k is a few sines, with NaNs where `gaps` says. It starts
   *  on 2026-01-01, or at 0 when `relative`; `time: 'index'` is a file with no time (a sample a second, as the core). */
  function recording(path, { channels = ['accel_x', 'accel_y', 'roll', 'pitch', 'motor_rpm', 'motor_rpm_target'], units, samples = 200000, rate = 1000, built = true, relative = false, time = 'rate' } = {}) {
    const chans = channels.map((name, k) => ({ name, unit: units?.[k] ?? (name.includes('rpm') ? 'rpm' : name.startsWith('accel') ? 'g' : 'deg'), dtype: 'f32' }));
    const index = time === 'index';
    return { path, chans, samples, rate: index ? 1 : rate, built, done: built ? samples : 0, progress: null, t0: index || relative ? 0n : T0, index };
  }
  // several files in one tab: a group is a composite recording (channels slot after slot, `·n` on the later files)
  // whose views, values and stats route to each file through its time map, as the core's `Group` does
  const kn = (knots) => (knots ?? []).map(([a, b]) => [BigInt(a), BigInt(b)]);
  function knotsAt(knots, x, back = false) {
    const k = kn(knots).map(([a, b]) => (back ? [b, a] : [a, b]));
    if (!k.length) return x;
    if (k.length === 1) return x - k[0][0] + k[0][1];
    let i = k.findIndex((p) => p[0] > x);
    i = i < 0 ? k.length - 1 : Math.max(1, i);
    return k[i - 1][1] + ((x - k[i - 1][0]) * (k[i][1] - k[i - 1][1])) / (k[i][0] - k[i - 1][0]);
  }
  function groupOut(id, o) {
    const g = o.group;
    let first = 0;
    const slots = g.members.map((m) => {
      const r = fake.open.get(m.file).r;
      const out = { file: m.file, first, count: r.chans.length, knots: m.knots, own: [String(timeAt(r, 0)), String(timeAt(r, r.samples - 1))] };
      first += r.chans.length;
      return out;
    });
    const a = slots.map((s) => knotsAt(s.knots, BigInt(s.own[0])));
    const b = slots.map((s) => knotsAt(s.knots, BigInt(s.own[1])));
    const lo = a.reduce((x, y) => (y < x ? y : x));
    const hi = b.reduce((x, y) => (y > x ? y : x));
    const p = fake.open.get(g.members[0].file).r;
    const info = { path: p.path, format: 'tlm', samples: p.samples, time: { kind: 'rate', hz: p.rate }, channels: o.r.chans, skipped: [] };
    return { id, info, range: [String(lo), String(hi)], status: { state: 'built' }, slots };
  }
  function groupRoute(o, ch) {
    let first = 0;
    for (let i = 0; i < o.group.members.length; i++) {
      const m = o.group.members[i];
      const r = fake.open.get(m.file).r;
      if (ch < first + r.chans.length) return { m, r, local: ch - first };
      first += r.chans.length;
    }
    return null;
  }
  // a stand-in for the core's parser: enough to tell a finished expression from a broken one, and which names it reads
  const FNS = new Set(['abs', 'sqrt', 'ln', 'log', 'log10', 'exp', 'pow', 'min', 'max', 'clamp', 'round', 'floor', 'ceil', 'sign', 'sin', 'cos', 'tan', 'delta', 'prev', 'pct_change', 'rate', 'cumsum', 'integral', 'sma', 'ema', 'rolling_std', 'median', 'lowpass', 'highpass', 'bandpass', 'mean', 'std', 'first', 'last', 'count', 'sum', 'if', 'interp', 'pi', 'e']);
  function exprError(expr) {
    let depth = 0;
    for (let i = 0; i < expr.length; i++) {
      if (expr[i] === '(') depth++;
      if (expr[i] === ')' && --depth < 0) return { msg: 'Unexpected ")"', at: i };
    }
    if (depth > 0) return { msg: 'Expected ")"', at: expr.length };
    if (/[+\-*/^%]\s*$/.test(expr)) return { msg: 'The expression ends too soon', at: expr.length };
    if (!expr.trim()) return { msg: 'The expression ends too soon', at: 0 };
    for (const m of expr.matchAll(/([A-Za-z_][A-Za-z0-9_.]*)\s*\(/g)) if (!FNS.has(m[1].toLowerCase())) return { msg: `There's no function called ${m[1].toLowerCase()}`, at: m.index };
    return null;
  }
  const exprNames = (expr) => [...expr.matchAll(/"([^"]+)"|\b([A-Za-z_][A-Za-z0-9_.]*)\b(?!\s*\()/g)].map((m) => m[1] ?? m[2]).filter((n) => !FNS.has(n) && !/^\d/.test(n));
  /** The library's recipes that apply to a recording (automatic, or attached by name) as channels after its own. */
  /** A channel adjusted with a missing rule has 7 samples (7 ms) missing, so the badge and Measure have something to show. */
  function missingOf(r, adjust) {
    r.missing = (adjust ?? [])
      .filter((a) => a.values?.length || a.lo != null || a.hi != null || a.when)
      .map((a) => ({ channel: r.chans.findIndex((c) => c.name === a.channel), count: 7, ns: 7e6 }))
      .filter((m) => m.channel >= 0);
    return r.missing;
  }
  /** A condition with "bad" in it is a mistake the core names. */
  function badWhen(adjust) {
    const w = (adjust ?? []).find((a) => /bad/.test(a.when ?? ''));
    if (w) throw `Missing when: There's no channel called bad (at 0)`;
  }
  function applyRecipes(r, attached, exclude = [], extra = []) {
    const named = [...new Set(attached)].map((n) => fake.recipes.find((x) => x.name === n)).filter(Boolean);
    const picked = named.concat(fake.recipes.filter((x) => x.automatic && !exclude.includes(x.name) && !named.includes(x)));
    const skipped = attached.filter((n) => !fake.recipes.some((x) => x.name === n)).map((name) => ({ name, why: 'There is no recipe with that name' }));
    const derived = [];
    for (const x of picked) {
      const missing = exprNames(x.expr).find((n) => !r.base.some((c) => c.name === n) && !extra.includes(n) && !fake.recipes.some((y) => y.name === n));
      if (missing) {
        if (attached.includes(x.name)) skipped.push({ name: x.name, why: `There's no channel called ${missing}` });
        continue;
      }
      r.chans.push({ name: x.name, unit: x.unit ?? null, dtype: 'f64', ...(x.category ? { category: x.category } : {}) });
      derived.push({ channel: r.chans.length - 1, name: x.name, expr: x.expr, automatic: !!x.automatic });
    }
    return { derived, skipped };
  }
  const dtNs = (r) => BigInt(Math.round(1e9 / r.rate));
  /** What a check gets: its entry in `fake.checkOutcomes`, else a pass (a rule: no events). The core's wire shapes. */
  const outcomeOf = (r, k) => {
    const o = fake.checkOutcomes[k.name] ?? {};
    const runs = (o.runs ?? []).map(([a, b]) => ({ start: String(timeAt(r, 0) + BigInt(Math.round(a * 1e9))), end: String(timeAt(r, 0) + BigInt(Math.round(b * 1e9))) }));
    const verdict = !k.enabled ? 'off' : (o.verdict ?? (k.kind === 'rule' ? 'mark' : 'pass'));
    const names = [...new Set([...(k.expect + ' ' + (k.when ?? '')).matchAll(/"[^"]+"|[A-Za-z_][A-Za-z0-9_]*/g)].map((m) => m[0].replace(/"/g, '')).filter((n) => r.chans.some((c) => c.name === n)))];
    return { name: k.name, verdict, count: runs.length, secs: runs.reduce((a, x) => a + Number(BigInt(x.end) - BigInt(x.start)) / 1e9, 0), runs, channels: o.channels ?? names, ...(o.worst != null ? { worst: o.worst, worst_t: runs[0]?.start } : {}), ...(o.value != null ? { value: o.value, function: o.function, limit: o.limit } : {}), ...(o.message ? { message: o.message } : {}) };
  };
  const RANK = { fail: 5, warn: 4, err: 3, cant: 2, pass: 1, na: 0, mark: 0, off: 0 };
  /** The same roll-up as the core's. */
  function rollupOf(set, outs) {
    const groups = [];
    set.check.forEach((k, i) => (groups.find((g) => g.name === k.group) ?? groups[groups.push({ name: k.group, os: [] }) - 1]).os.push(outs[i]));
    const worst = (vs) => vs.reduce((a, b) => (RANK[b] > RANK[a] ? b : a), 'pass');
    const rolls = groups.map(({ name, os }) => {
      const counted = os.map((o) => o.verdict).filter((v) => !['na', 'mark', 'off'].includes(v));
      const n = (v) => os.filter((o) => o.verdict === v).length;
      const verdict = counted.length ? worst(counted) : os.some((o) => o.verdict === 'mark') ? 'mark' : os.length && os.every((o) => o.verdict === 'off') ? 'off' : 'na';
      return { name, verdict, events: os.filter((o) => o.verdict === 'mark').reduce((a, o) => a + o.count, 0), info: set.info_groups.includes(name), total: counted.length, failed: n('fail'), warned: n('warn'), passed: n('pass') };
    });
    const main = rolls.filter((g) => !g.info && !['na', 'mark', 'off'].includes(g.verdict));
    return { groups: rolls, verdict: main.length ? worst(main.map((g) => g.verdict)) : rolls.some((g) => g.verdict === 'mark') ? 'mark' : 'na', failed: outs.filter((o) => o.verdict === 'fail').length, warned: outs.filter((o) => o.verdict === 'warn').length, events: rolls.reduce((a, g) => a + g.events, 0), applies: outs.some((o) => !['cant', 'err', 'off'].includes(o.verdict)) };
  }
  function checkRun(r, set) {
    const outcomes = set.check.map((k) => outcomeOf(r, k));
    const events = [];
    set.check.forEach((k, i) =>
      outcomes[i].runs.forEach((s, n) => events.push({ id: `${k.kind}:${set.name}/${k.name}:${n}`, name: k.name, start: s.start, duration: String(BigInt(s.end) - BigInt(s.start)), category: k.kind === 'rule' ? (k.category ?? '') : 'check', source: k.kind === 'rule' ? 'rule' : 'check', channels: outcomes[i].channels })),
    );
    events.sort((a, b) => (BigInt(a.start) < BigInt(b.start) ? -1 : 1));
    return { set: set.name, result: { set: set.name, outcomes, rollup: rollupOf(set, outcomes) }, events };
  }
  const timeAt = (r, i) => (r.t0 ?? T0) + BigInt(i) * dtNs(r);
  const valueAt = (r, ch, i) => {
    const s = i / r.rate;
    return Math.fround((ch + 1) * Math.sin(s * (0.05 + 0.031 * ch)) + 0.3 * Math.sin(s * (1.7 + ch)));
  };
  const indexAt = (r, t) => Number((BigInt(t) - (r.t0 ?? T0)) / dtNs(r));
  const clampI = (r, i) => Math.max(0, Math.min(r.samples - 1, i));

  /** One channel over a viewport: raw samples (and one neighbour each side) up to 4W, else M4 per pixel column. */
  function reduce(r, ch, vp, upTo) {
    const lim = Math.min(r.samples, upTo);
    const a = clampI(r, indexAt(r, vp.t0));
    const b = Math.min(lim - 1, Math.max(a, Math.ceil(Number(BigInt(vp.t1) - (r.t0 ?? T0)) / Number(dtNs(r)))));
    const t = [];
    const y = [];
    if (lim <= 0 || a >= lim) return { kind: 'raw', t, y };
    if (b - a + 1 <= Math.max(4 * vp.widthPx, vp.rawMax ?? 0)) {
      for (let i = a; i <= b; i++) [t[t.length], y[y.length]] = [timeAt(r, i), valueAt(r, ch, i)];
      return { kind: 'raw', t, y };
    }
    const per = (b - a + 1) / vp.widthPx;
    for (let c = 0; c < vp.widthPx; c++) {
      const i0 = a + Math.floor(c * per);
      const i1 = Math.min(b, a + Math.floor((c + 1) * per) - 1);
      if (i1 < i0) continue;
      let lo = i0;
      let hi = i0;
      const step = Math.max(1, Math.floor((i1 - i0) / 16));
      for (let i = i0; i <= i1; i += step) {
        if (valueAt(r, ch, i) < valueAt(r, ch, lo)) lo = i;
        if (valueAt(r, ch, i) > valueAt(r, ch, hi)) hi = i;
      }
      for (const i of [...new Set([i0, Math.min(lo, hi), Math.max(lo, hi), i1])]) [t[t.length], y[y.length]] = [timeAt(r, i), valueAt(r, ch, i)];
    }
    return { kind: 'm4', t, y };
  }

  /** The fake's one time jump, as the core shows it: a NaN before the jump, or the line joined and the join listed. */
  function gaps(r, e, tr) {
    if (fake.gapAt == null) return tr;
    const at = timeAt(r, fake.gapAt);
    const t = [];
    const y = [];
    const dashed = [];
    tr.t.forEach((ts, i) => {
      if (i && tr.t[i - 1] < at && ts >= at) {
        if (e.connectGaps) dashed.push(t.length);
        else [t[t.length], y[y.length]] = [tr.t[i - 1] + 1n, NaN];
      }
      [t[t.length], y[y.length]] = [ts, tr.y[i]];
    });
    return { ...tr, t, y, dashed };
  }

  // ---------- IPC ----------

  const fake = {
    /** Windows the app opened (name, what they open with, where). */
    windows: [],
    cache: [{ bytes: 900 * 2 ** 20, open: false }, { bytes: 300 * 2 ** 20, open: false }, { bytes: 64 * 2 ** 20, open: true }],
    files: {},
    launch: null,
    pick: null,
    /** What Open folder returns, and folders that exist without recordings. */
    pickFolder: null,
    folders: [],
    unopenable: [],
    /** Live sources by endpoint: a recording spec, plus `already` (seconds recorded when it connects). */
    sources: {},
    /** What the live commands were asked: connects, and triggers armed. A trigger fires this long after it is armed (ms). */
    liveConnects: [],
    triggers: [],
    triggerAfter: 1000,
    /** Records the live sources report dropped. */
    liveDropped: 0,
    calls: [],
    /** Shared views by folder and file, folders that can't be written, what Save… dialogs return, exports written. */
    views: {},
    readOnly: [],
    savePick: null,
    exported: [],
    dataExports: [],
    saved_files: [],
    exportFails: null,
    /** Rows each `rows` call returned. */
    rowsAsked: [],
    queries: new Map(),
    masks: new Map(),
    /** The runs of time each mask leaves out, [t0, t1] ns, by owner. */
    maskRuns: new Map(),
    marksAsked: [],
    maskCalls: [],
    queriesRun: [],
    /** The precision each `view` asked for. */
    precisions: [],
    /** A time jump the fake recording has (a sample index: the line breaks before it), and each view entry's connectGaps. */
    gapAt: null,
    connectAsked: [],
    open: new Map(),
    /** The recipe library (name, expr, unit, category, decimals, automatic); every recipe_save, derive_apply, check and spectrum asked. */
    recipes: [],
    recipeSaves: [],
    derivedApplied: [],
    /** Events by file path, and every event saved. */
    events: {},
    eventSaves: [],
    /** The core's check sets, what each check gets on a file (by check name: { verdict, runs: [[from, to] seconds after the start], worst, value, function, limit, message, channels }), the runs and previews it was asked for. */
    checkSets: [],
    /** Several files in one tab: each group_set's files, group_map and align_estimate calls, and what a method returns instead of the default (`{ knots, residuals, quality, note, channel }`, or an Error to fail). */
    /** Shared configuration: the Share dialog's rows (`cfgRows`), what the folder gives (`cfgFrom`: categories and widgets; `cfgPull`: what Use brings back), the commands seen, and the saved time maps. */
    cfgRows: [],
    cfgWritable: true,
    cfgGit: false,
    cfgProblems: [],
    cfgFrom: { categories: [], widgets: [] },
    cfgRecipes: [],
    cfgChecks: [],
    cfgPull: { from: { categories: [], widgets: [] }, views: [] },
    cfgCalls: [],
    timemaps: [],
    groupSets: [],
    groupMaps: [],
    aligns: [],
    alignResults: {},
    checkOutcomes: {},
    checkRuns: [],
    checkPreviews: [],
    checkSetSaves: [],
    /** The adjustments (scale, offset, missing) each open or adjust_apply was given. */
    adjusts: [],
    checks: [],
    spectra: [],
    /** What every widgets_frame and xy_points call asked for. */
    frames: [],
    xyAsks: [],
    /** Values by channel name that replace the synthetic ones in widgets_frame (a state, a heading), as a function of the sample. */
    frameValue: {},
    /** CSV files the import dialog sees, by path: `fake.csv(path, names, opts)`; the profiles saved by header, what
     * each open was given, and every profile_save. */
    csvs: {},
    profiles: {},
    openedWith: [],
    saved: [],
    csv(path, names, o = {}) {
      const time = o.time === undefined ? 0 : o.time;
      const columns = names.map((n, i) => ({ source: n, name: n, unit: o.units?.[n] ?? null, kind: i === time ? 'time' : (o.text ?? []).includes(n) ? 'text' : 'number' }));
      fake.csvs[path] = { header_key: [...names].sort().join('\u0000'), delimiter: o.delimiter ?? ',', columns, time, time_unit: time == null ? null : (o.unit ?? 's'), preview: [], rows: o.rows ?? 1000, rows_exact: true, rate_hz: time == null ? null : (o.rate ?? 10) };
    },
    /** Binary files for the descriptor form, by path: `fake.bin(path)` is a logger's 24-byte frames (sync A55A, a
     * counter, a µs clock, three readings, CRC-16); descriptors saved to the cache and shared to folders; every
     * descriptor_preview and descriptor_save the page sent. The fake decodes a little (fixed frames, integers) so the
     * form has something to show: the real wording and values are the core's, tested in Rust. */
    bins: {},
    decoders: {},
    sharedDecoders: {},
    previewed: [],
    /** Every open: its path and packet type. */
    opened: [],
    savedDescriptors: [],
    bin(path, o = {}) {
      const n = o.frames ?? 64;
      const out = new Uint8Array(n * 24);
      const dv = new DataView(out.buffer);
      for (let k = 0; k < n; k++) {
        const at = k * 24;
        out[at] = 0xa5;
        out[at + 1] = 0x5a;
        dv.setUint16(at + 2, k, true);
        dv.setUint32(at + 4, 120000000 + k * 10000, true);
        dv.setUint16(at + 8, 1200 + k, true);
        out[at + 10] = 60 + (k & 7);
        dv.setUint16(at + 14, 24600 - k, true);
        dv.setUint16(at + 22, crc16(out.subarray(at, at + 22)), true);
      }
      fake.bins[path] = { bytes: o.bytes ?? out, spec: o.spec ?? { channels: ['motor_rpm', 'battery_v'], samples: 4000, rate: 100 } };
    },
    recording,
    /** Moves `path`'s pyramid build to `fraction` (1 finishes it), sending progress like the core. */
    build(path, fraction) {
      for (const [, o] of fake.open) {
        if (o.r.path !== path || !o.progress) continue;
        o.r.done = Math.round(o.r.samples * Math.min(1, fraction));
        o.r.built = o.r.done >= o.r.samples;
        send(o.progress, { done: o.r.done, total: o.r.samples, builtUntil: o.r.done ? String(timeAt(o.r, o.r.done - 1)) : null, error: null });
      }
    },
    /** Another window saved these shared settings (the event the core sends the others). */
    otherWindowSaved(settings) {
      const text = JSON.stringify(settings);
      fake.sharedSettings = text;
      emit('settings-changed', { from: 'win-2', text });
    },
    drop(paths) {
      for (const ev of ['tauri://drag-enter', 'tauri://drag-drop']) emit(ev, { paths, position: { x: 200, y: 200 } });
    },
  };
  window.__tlmFake = fake;

  const callbacks = new Map();
  let nextCb = 1;
  // ---------- the descriptor form's stand-in for the core ----------
  function crc16(b) {
    let c = 0xffff;
    for (const x of b) {
      c ^= x << 8;
      for (let k = 0; k < 8; k++) c = c & 0x8000 ? ((c << 1) ^ 0x1021) & 0xffff : (c << 1) & 0xffff;
    }
    return c;
  }
  const SIZES = { i8: 1, u8: 1, i16: 2, u16: 2, i32: 4, u32: 4, i64: 8, u64: 8, f32: 4, f64: 8 };
  const globOk = (g, name) => new RegExp('^' + g.split('*').map((x) => x.replace(/[.+?^${}()|[\]\\]/g, '\\$&')).join('.*') + '$').test(name);
  const slug = (n) => (String(n || '').toLowerCase().split(/[^a-z0-9]+/).filter(Boolean).join('-') || 'decoder');
  const dirOf = (p) => p.replace(/\/[^/]*$/, '');
  const nameOf = (p) => p.split('/').pop();
  const known = (path) => [
    ...Object.entries(fake.sharedDecoders[dirOf(path)] ?? {}).map(([id, descriptor]) => ({ id, path: `${dirOf(path)}/.tlmbench/decoders/${id}.toml`, shared: true, descriptor })),
    ...Object.entries(fake.decoders).map(([id, descriptor]) => ({ id, path: `/cache/decoders/${id}.toml`, shared: false, descriptor })),
  ].map((k) => ({ ...k, claims: (k.descriptor.files ?? []).some((g) => globOk(g, nameOf(path))) }));
  const claimed = (path) => known(path).some((k) => k.claims);
  const problemOf = (d) => {
    const hs = d.packets?.header_size ?? 0;
    const ext = d.record_size + hs;
    if (!(d.record_size >= 1)) return `record_size ${d.record_size} must be between 1 and 1048576 bytes`;
    if (!d.fields.length) return 'no fields';
    for (const f of d.fields) if (f.offset + SIZES[f.type] > ext) return `field "${f.name}" (${f.type} at offset ${f.offset}) runs past the ${ext}-byte ${hs ? 'packet header and record' : 'record'}`;
    for (const f of d.fields) if (f.bits && f.bits.shift + f.bits.width > SIZES[f.type] * 8) return `field "${f.name}": bits shift ${f.bits.shift} and width ${f.bits.width} don't fit in ${SIZES[f.type] * 8} bits`;
    if (d.time.source !== 'rate' && d.time.source !== 'index' && !d.fields.some((f) => f.name === d.time.field)) return `time.field "${d.time.field}" is not one of the fields`;
    return null;
  };
  function previewOf(bytes, d) {
    const toml = `# the fake core's TOML\nname = ${JSON.stringify(d.name)}\n${d.packets ? `[packets]\nsync = ${JSON.stringify(d.packets.sync ?? '')}\n` : ''}`;
    const problem = problemOf(d);
    const out = { toml, problem, error: null, rows: [], stride: 0, start: 0, skipped: 0, bad_checks: 0, checked: false, hex_at: 0, hex: [] };
    if (problem) return out;
    const le = d.endian !== 'big';
    const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.length);
    const read = (o, t) => ({ i8: () => dv.getInt8(o), u8: () => dv.getUint8(o), i16: () => dv.getInt16(o, le), u16: () => dv.getUint16(o, le), i32: () => dv.getInt32(o, le), u32: () => dv.getUint32(o, le), i64: () => Number(dv.getBigInt64(o, le)), u64: () => Number(dv.getBigUint64(o, le)), f32: () => dv.getFloat32(o, le), f64: () => dv.getFloat64(o, le) })[t]();
    const p = d.packets;
    const hs = p?.header_size ?? 0;
    const cks = { none: 0, sum8: 1, 'crc16-ccitt': 2, crc32: 4 }[p?.checksum ?? 'none'];
    const step = d.record_size + hs + cks;
    const sync = (p?.sync ?? '').match(/../g)?.map((x) => parseInt(x, 16)) ?? [];
    let o = d.header_size;
    while (sync.length && o < bytes.length && !sync.every((x, j) => bytes[o + j] === x)) o++;
    out.skipped = sync.length ? o - d.header_size : 0;
    out.start = o;
    out.stride = step;
    out.checked = cks > 0;
    for (let k = 0; o + step <= bytes.length && k < 8; k++, o += step) {
      const ok = cks ? (le ? dv.getUint16(o + step - 2, true) === crc16(bytes.subarray(o, o + step - 2)) : false) : null;
      const base = o;
      const values = d.fields.map((f) => { const raw = read(base + f.offset, f.type); const v = f.bits ? (raw >> f.bits.shift) & ((1 << f.bits.width) - 1) : raw; return v * (f.scale ?? 1) + (f.value_offset ?? 0); });
      const ti = d.fields.findIndex((f) => f.name === d.time.field);
      const t = d.time.source === 'rate' ? k / d.time.rate_hz : d.time.source === 'index' ? k : d.time.source === 'field' ? (read(base + d.fields[ti].offset, d.fields[ti].type) * { ns: 1e-9, us: 1e-6, ms: 1e-3, s: 1 }[d.time.unit]) : read(base + d.fields[ti].offset, d.fields[ti].type) / d.time.clock_hz;
      out.rows.push({ offset: base, time: t, values, ok });
    }
    if (!out.rows.length) out.error = 'No whole record found in the first bytes: check the framing.';
    out.bad_checks = out.rows.filter((r) => r.ok === false).length;
    out.hex_at = out.rows[0]?.offset ?? 0;
    out.hex = [...bytes.subarray(out.hex_at, out.hex_at + 64)];
    return out;
  }
  // the filter language: atoms joined by `and`, groups joined by `or`
  const parseFilter = (text) => {
    const s = String(text || '').trim().toLowerCase();
    if (!s) return { test: () => true };
    const num = '(-?\\d*\\.?\\d+(?:e-?\\d+)?)';
    const atoms = [];
    for (const g of s.split(/\s+or\s+/)) {
      const tests = [];
      for (const part of g.split(/\s+and\s+/)) {
        const x = part.trim();
        let m;
        if (x === 'blank') tests.push((v) => Number.isNaN(v));
        else if ((m = x.match(new RegExp(`^${num}\\s*\\.\\.\\s*${num}$`)))) tests.push((v) => v >= +m[1] && v <= +m[2]);
        else if ((m = x.match(new RegExp(`^(>=|<=|!=|>|<|=)?\\s*${num}$`)))) {
          const a = +m[2];
          const op = m[1] || '=';
          tests.push((v) => ({ '>': v > a, '<': v < a, '>=': v >= a, '<=': v <= a, '=': Math.abs(v - a) < 1e-9, '!=': !Number.isNaN(v) && Math.abs(v - a) >= 1e-9 })[op]);
        } else return { error: `can't read "${x}"` };
      }
      atoms.push(tests);
    }
    return { test: (v) => atoms.some((g) => g.every((t) => t(v))) };
  };
  const listeners = new Map();
  const send = (ch, message) => callbacks.get(ch.id)?.({ message, index: ch.next++ });
  const emit = (event, payload) => (listeners.get(event) || []).forEach((h) => callbacks.get(h)?.({ event, id: h, payload }));

  let nextFile = 1;
  const commands = {
    launch: () => ({ file: fake.launch, gate: null, gateMode: null }),
    needs_descriptor: ({ path }) => !!fake.bins[path] && !claimed(path),
    descriptor_list: ({ path }) => known(path),
    descriptor_preview({ path, descriptor }) {
      fake.previewed.push(descriptor);
      return previewOf(fake.bins[path].bytes, descriptor);
    },
    descriptor_find_frames({ path }) {
      const b = fake.bins[path].bytes;
      for (let w = 2; w >= 1; w--) for (let stride = 4; stride <= 256; stride++) {
        if (stride * 8 > b.length) break;
        if ([...b.subarray(0, w)].every((x) => x === 0)) continue;
        let ok = true;
        for (let k = 1; k < 8 && ok; k++) for (let j = 0; j < w; j++) ok &&= b[k * stride + j] === b[j];
        if (ok) return { sync: [...b.subarray(0, w)].map((x) => x.toString(16).padStart(2, '0').toUpperCase()).join(''), stride, start: 0 };
      }
      return null;
    },
    descriptor_save({ descriptor, replace }) {
      const problem = problemOf(descriptor);
      if (problem) throw problem;
      fake.savedDescriptors.push({ descriptor, replace });
      let id = slug(descriptor.name);
      if (replace && fake.decoders[replace]) {
        if (slug(fake.decoders[replace].name) === id) id = replace;
        else delete fake.decoders[replace];
      } else for (let n = 2; fake.decoders[id]; n++) id = `${slug(descriptor.name)}-${n}`;
      fake.decoders[id] = descriptor;
      return { id, path: `/cache/decoders/${id}.toml`, shared: false, descriptor, claims: false };
    },
    descriptor_share({ path, descriptor }) {
      if (fake.readOnly.includes(dirOf(path))) throw `can't write to ${dirOf(path)}/.tlmbench/decoders: Read-only file system (os error 30)`;
      const id = slug(descriptor.name);
      (fake.sharedDecoders[dirOf(path)] ??= {})[id] = descriptor;
      return `${dirOf(path)}/.tlmbench/decoders/${id}.toml`;
    },
    import_inspect({ paths }) {
      return paths.map((path) => {
        const insp = fake.csvs[path];
        if (!insp) return { path, inspection: null, error: 'not a CSV, TSV or TXT file', found: { exact: null, near: null } };
        const names = insp.columns.map((c) => c.source);
        const exact = fake.profiles[insp.header_key]?.profile ?? null;
        let near = null;
        if (!exact) {
          for (const saved of Object.values(fake.profiles)) {
            const share = names.filter((n) => saved.header.includes(n)).length / Math.max(names.length, saved.header.length);
            if (share >= 0.6 && (!near || share > near[0])) near = [share, saved];
          }
        }
        return { path, inspection: insp, error: null, found: { exact, near } };
      });
    },
    profile_save({ header, profile }) {
      fake.saved.push({ header, profile });
      fake.profiles[[...header].sort().join('\u0000')] = { header, profile };
      return null;
    },
    // the packet types of the descriptor that names the file (each opens as its own recording)
    packet_parts: ({ path }) => (known(path).find((k) => k.claims)?.descriptor.packet_types ?? []).map((t) => t.name),
    open({ path, profile: given, part, recipes, exclude, adjust, progress }) {
      fake.adjusts.push(adjust ?? []);
      badWhen(adjust);
      fake.opened.push({ path, part: part ?? null });
      // the core applies the header's saved profile when none is given
      const profile = given ?? fake.profiles[fake.csvs[path]?.header_key]?.profile ?? null;
      fake.openedWith.push({ path, profile });
      if (fake.bins[path] && !fake.files[path]) {
        if (!claimed(path)) throw `${path}: not a format tlmbench knows`;
        fake.files[path] = fake.bins[path].spec;
      }
      let spec = fake.files[path];
      const types = known(path).find((k) => k.claims)?.descriptor.packet_types ?? [];
      if (types.length && fake.bins[path]) {
        if (part == null) throw `${path} holds ${types.length} packet types (${types.map((t) => t.name).join(', ')}): say which one to open`;
        spec = fake.bins[path].spec.parts?.[part] ?? fake.bins[path].spec;
      }
      if (!spec) throw `couldn't open ${path}: no such file`;
      const r = recording(path, spec);
      // a profile's names, units and categories land on the channels in order (the core does this when it decodes)
      const numeric = (fake.csvs[path]?.columns ?? []).filter((c) => c.kind === 'number').map((c) => c.source);
      r.chans.forEach((c, k) => {
        const ch = profile?.columns?.find((x) => x.source === numeric[k]);
        if (ch?.name) c.name = ch.name;
        if (ch?.unit != null) c.unit = ch.unit || null;
        if (ch?.category) c.category = ch.category;
      });
      const id = nextFile++;
      const ch = progress && { id: progress.id ?? Number(String(progress).replace(/\D/g, '')), next: 0 };
      fake.open.set(id, { r, progress: ch });
      r.base = r.chans.slice();
      const { derived, skipped } = applyRecipes(r, recipes ?? [], exclude ?? []);
      const info = { path, ...(part ? { part } : {}), format: path.endsWith('.csv') ? 'csv' : 'tlm', samples: r.samples, time: r.index ? { kind: 'index' } : { kind: 'rate', hz: r.rate }, channels: r.chans, skipped: [] };
      return { id, info, range: [String(timeAt(r, 0)), String(timeAt(r, r.samples - 1))], status: r.built ? { state: 'built' } : { state: 'building', done: 0, total: r.samples }, derived, skipped, missing: missingOf(r, adjust) };
    },
    // derived channels: the library (`recipes`), the recipes each open or apply was given, and what a check or spectrum was asked
    /** The events you added, by file path (the core keeps them per recording in the app cache). */
    events_list({ file }) {
      return fake.events[fake.open.get(file).r.path] ?? [];
    },
    event_save({ file, event }) {
      const path = fake.open.get(file).r.path;
      const all = (fake.events[path] ?? []).filter((e) => e.id !== event.id).concat({ source: 'user', channels: [], ...event });
      all.sort((a, b) => (BigInt(a.start) < BigInt(b.start) ? -1 : 1));
      fake.events[path] = all;
      fake.eventSaves.push(event);
      return all;
    },
    event_delete({ file, id }) {
      const path = fake.open.get(file).r.path;
      fake.events[path] = (fake.events[path] ?? []).filter((e) => e.id !== id);
      return fake.events[path];
    },
    check_sets_list: () => JSON.parse(JSON.stringify(fake.checkSets)),
    check_set_save({ set, replacing }) {
      fake.checkSetSaves.push({ set: JSON.parse(JSON.stringify(set)), replacing: replacing ?? null });
      const k = fake.checkSets.findIndex((x) => x.name === (replacing ?? set.name));
      if (k >= 0) fake.checkSets[k] = JSON.parse(JSON.stringify(set));
      else fake.checkSets.push(JSON.parse(JSON.stringify(set)));
      return null;
    },
    check_set_delete({ name }) {
      fake.checkSets = fake.checkSets.filter((s) => s.name !== name);
      return null;
    },
    checks_run({ file, sets }) {
      const r = fake.open.get(file).r;
      fake.checkRuns.push({ path: r.path, sets: sets.slice() });
      return sets.map((n) => fake.checkSets.find((s) => s.name === n)).filter(Boolean).map((set) => checkRun(r, set));
    },
    /** A file that isn't open, checked by the folder run: its outcomes are `fake.checkFileOutcomes[path][check name]`, else the general ones. */
    checks_run_path({ path, sets }) {
      const r = recording(path, fake.files[path] ?? {});
      if (fake.checkFileTimes?.[path]) r.t0 = BigInt(fake.checkFileTimes[path]);
      fake.checkRuns.push({ path, sets: sets.slice(), folder: true });
      const saved = fake.checkOutcomes;
      fake.checkOutcomes = { ...saved, ...(fake.checkFileOutcomes?.[path] ?? {}) };
      try {
        const out = sets.map((n) => fake.checkSets.find((s) => s.name === n)).filter(Boolean).map((set) => ({ ...checkRun(r, set), t_first: String(timeAt(r, 0)) }));
        if (fake.checkFileOutcomes?.[path]?.__error) throw fake.checkFileOutcomes[path].__error;
        return out;
      } finally {
        fake.checkOutcomes = saved;
      }
    },
    write_text_file({ path, text }) {
      fake.written = { path, text };
      return null;
    },
    check_preview({ file, check }) {
      const r = fake.open.get(file).r;
      fake.checkPreviews.push(JSON.parse(JSON.stringify(check)));
      return outcomeOf(r, check);
    },
    recipes_list: () => fake.recipes.map((r) => ({ ...r })),
    recipe_save({ recipe, replacing }) {
      const bad = exprError(recipe.expr);
      if (bad) throw `${bad.msg} (at ${bad.at})`;
      const k = fake.recipes.findIndex((r) => r.name === (replacing ?? recipe.name));
      if (k >= 0) fake.recipes[k] = { ...recipe };
      else fake.recipes.push({ ...recipe });
      fake.recipeSaves.push({ recipe: { ...recipe }, replacing: replacing ?? null });
      return null;
    },
    recipe_delete({ name }) {
      fake.recipes = fake.recipes.filter((r) => r.name !== name);
      return null;
    },
    recipes_export: ({ names }) => `schema = 1\n${fake.recipes.filter((r) => !names.length || names.includes(r.name)).map((r) => `\n[[recipe]]\nname = "${r.name}"\nexpr = "${r.expr}"\nautomatic = ${r.automatic}\n`).join('')}`,
    recipes_import({ text }) {
      if (!/schema = 1/.test(text)) throw 'recipe file schema is not supported';
      const names = [...text.matchAll(/name = "([^"]+)"\s*\nexpr = "([^"]+)"(?:\s*\n(?:unit = "[^"]*"\s*\n)?automatic = (true|false))?/g)];
      names.forEach((m) => (fake.recipes = fake.recipes.filter((r) => r.name !== m[1]).concat({ name: m[1], expr: m[2], automatic: m[3] !== 'false' })));
      return names.map((m) => m[1]);
    },
    set_expr_env({ labels, limits }) {
      fake.exprEnv = { labels, limits };
    },
    /** An open file opened again with other adjustments: every open or apply's list is in `adjusts`. */
    adjust_apply({ file, adjust, recipes, exclude }) {
      const o = fake.open.get(file);
      if (!o) throw 'that file is not open';
      fake.adjusts.push(adjust ?? []);
      badWhen(adjust);
      const r = o.r;
      r.chans = r.base.slice();
      const { derived, skipped } = applyRecipes(r, recipes, exclude ?? []);
      const info = { path: r.path, format: 'tlm', samples: r.samples, time: r.index ? { kind: 'index' } : { kind: 'rate', hz: r.rate }, channels: r.chans, skipped: [] };
      return { id: file, info, range: [String(timeAt(r, 0)), String(timeAt(r, r.samples - 1))], status: { state: 'built' }, derived, skipped, missing: missingOf(r, adjust) };
    },
    missing_between({ file, channels }) {
      const o = fake.open.get(file);
      if (!o) throw 'that file is not open';
      return channels.filter((c) => (o.r.missing ?? []).some((m) => m.channel === c)).map((c) => ({ channel: c, count: 3, ns: 3e6 }));
    },
    derive_apply({ file, recipes, exclude, group, progress }) {
      const o = fake.open.get(file);
      if (!o) throw 'that file is not open';
      const r = o.r;
      r.chans = r.base.slice();
      // the other files of the tab: their channels by the names the tab gives them
      const extra = (fake.open.get(group)?.group?.members ?? []).slice(1).flatMap((m, i) => fake.open.get(m.file).r.chans.map((c) => `${c.name}·${i + 2}`));
      const { derived, skipped } = applyRecipes(r, recipes, exclude ?? [], extra);
      fake.derivedApplied.push({ file, recipes, exclude, group });
      const info = { path: r.path, format: 'tlm', samples: r.samples, time: r.index ? { kind: 'index' } : { kind: 'rate', hz: r.rate }, channels: r.chans, skipped: [] };
      return { id: file, info, range: [String(timeAt(r, 0)), String(timeAt(r, r.samples - 1))], status: { state: 'built' }, derived, skipped, missing: r.missing ?? [] };
    },
    derive_check({ file, expr }) {
      const o = fake.open.get(file);
      if (!o) throw 'that file is not open';
      fake.checks.push(expr);
      const bad = exprError(expr) ?? exprNames(expr).filter((n) => !o.r.chans.some((c) => c.name === n)).map((n) => ({ msg: `There's no channel called ${n}`, at: expr.indexOf(n) }))[0];
      if (bad) return { preview: null, error: bad };
      return { preview: { samples: Math.min(o.r.samples, 1048576), values: Math.min(o.r.samples, 1048576), min: -2.5, max: 2.5, mean: 0.1, number: /^\s*(mean|max|min)\(/.test(expr) ? 0.1 : null }, error: null };
    },
    spectrum({ file, channels, t0, t1 }) {
      const o = fake.open.get(file);
      if (!o) throw 'that file is not open';
      fake.spectra.push({ file, channels, t0, t1 });
      if (fake.spectrumNone) return [];
      const n = 1024;
      return channels.map((channel) => ({ channel, df: o.r.rate / n, nyquist: o.r.rate / 2, peak_hz: 12.5 * (channel + 1), peak_amp: 1 + channel, n, mag: Array.from({ length: n / 2 + 1 }, (_, k) => 1 / (1 + k) + (k === 13 * (channel + 1) ? 1 + channel : 0)) }));
    },
    // a Gauges pane's frame: the value at t, min and max from the start of the view to t, the run start and a trace
    widgets_frame({ file, t, view, widgets }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      fake.frames.push({ file, t, view, widgets });
      const val = (ch, i) => (fake.frameValue[r.chans[ch]?.name] ? fake.frameValue[r.chans[ch].name](i) : valueAt(r, ch, i));
      const i = clampI(r, indexAt(r, t));
      const a = clampI(r, indexAt(r, view.t0));
      const b = clampI(r, indexAt(r, view.t1));
      return {
        index: i,
        t: String(timeAt(r, i)),
        widgets: widgets.map((w) => {
          const ch = Math.min(w.channel, r.chans.length - 1);
          let [min, max] = [null, null];
          if (w.minmax && r.built) {
            [min, max] = [Infinity, -Infinity];
            for (let k = Math.min(a, i); k <= i; k += Math.max(1, Math.floor((i - a) / 400))) [min, max] = [Math.min(min, val(ch, k)), Math.max(max, val(ch, k))];
          }
          let runStart = null;
          if (w.run) {
            let k = i;
            while (k > 0 && val(ch, k - 1) === val(ch, i)) k--;
            runStart = String(timeAt(r, k));
          }
          const trace = [];
          if (w.trace) for (let q = 0; q < w.trace; q++) trace.push(q / (w.trace - 1), val(ch, Math.round(a + ((b - a) * q) / (w.trace - 1))));
          return { value: val(ch, i), min, max, runStart, trace };
        }),
      };
    },
    // an XY pane's points: pairs by index, thinned to maxPoints; the range of the pairs in the span
    xy_points({ file, span, ask }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      fake.xyAsks.push({ file, span, ask });
      const a = clampI(r, indexAt(r, span.t0));
      const b = clampI(r, indexAt(r, span.t1));
      const n = Math.max(1, Math.min(ask.maxPoints ?? 100000, b - a + 1));
      const val = (ch, i) => (fake.frameValue[r.chans[ch]?.name] ? fake.frameValue[r.chans[ch].name](i) : valueAt(r, ch, i));
      const series = ask.ys.map((y) => {
        const idx = Array.from({ length: n }, (_, q) => a + Math.round(((b - a) * q) / Math.max(1, n - 1)));
        return { y, t: idx.map((i) => timeAt(r, i)), x: idx.map((i) => val(ask.x, i)), ys: idx.map((i) => val(y, i)) };
      });
      const all = series.flatMap((s) => s.x.map((x, q) => [x, s.ys[q]]));
      const buf = new ArrayBuffer(48 + series.reduce((m, s) => m + 8 + 24 * s.x.length, 0));
      const v = new DataView(buf);
      [...'TLMX'].forEach((c, k) => v.setUint8(k, c.charCodeAt(0)));
      v.setUint32(4, series.length, true);
      v.setUint32(8, (all.length ? 2 : 0) | (ask.maxPoints && b - a + 1 > ask.maxPoints ? 1 : 0), true);
      const rg = all.length ? [Infinity, -Infinity, Infinity, -Infinity] : [NaN, NaN, NaN, NaN];
      for (const [x, y] of all) [rg[0], rg[1], rg[2], rg[3]] = [Math.min(rg[0], x), Math.max(rg[1], x), Math.min(rg[2], y), Math.max(rg[3], y)];
      rg.forEach((x, k) => v.setFloat64(16 + 8 * k, x, true));
      let at = 48;
      for (const s of series) {
        v.setUint32(at, s.y, true);
        v.setUint32(at + 4, s.x.length, true);
        at += 8;
        s.t.forEach((t, q) => v.setBigInt64(at + 8 * q, t, true));
        s.x.forEach((x, q) => v.setFloat64(at + 8 * s.x.length + 8 * q, x, true));
        s.ys.forEach((y, q) => v.setFloat64(at + 16 * s.x.length + 8 * q, y, true));
        at += 24 * s.x.length;
      }
      return buf;
    },
    group_set({ group, members }) {
      const rs = members.map((m) => fake.open.get(m.file)?.r);
      if (rs.some((r) => !r)) throw 'that file is not open';
      const chans = rs.flatMap((r, i) => r.chans.map((c) => ({ ...c, name: i ? `${c.name}·${i + 1}` : c.name })));
      const comp = Object.assign(Object.create(rs[0]), { chans });
      const id = group ?? nextFile++;
      const o = { r: comp, group: { members: members.map((m) => ({ file: m.file, knots: m.knots })) } };
      fake.open.set(id, o);
      fake.groupSets.push(members.map((m) => m.file));
      return groupOut(id, o);
    },
    group_state({ group }) {
      return groupOut(group, fake.open.get(group));
    },
    group_map({ group, slot, knots }) {
      const o = fake.open.get(group);
      o.group.members[slot].knots = knots;
      fake.groupMaps.push({ slot, knots });
      return groupOut(group, o);
    },
    align_estimate({ group, slot, method }) {
      const o = fake.open.get(group);
      fake.aligns.push({ slot, method });
      const rf = fake.open.get(o.group.members[0].file).r;
      const tf = fake.open.get(o.group.members[slot].file).r;
      const rmap = o.group.members[0].knots;
      const res = fake.alignResults[method.method];
      if (res instanceof Error) throw res.message;
      if (res) return { knots: res.knots, residuals: res.residuals ?? [], quality: res.quality ?? 1, note: res.note ?? null, channel: res.channel ?? null };
      switch (method.method) {
        case 'starts': return { knots: [['0', String(knotsAt(rmap, timeAt(rf, 0)) - timeAt(tf, 0))]], residuals: [], quality: 1, note: null, channel: null };
        case 'clock': return { knots: rmap, residuals: [], quality: 1, note: null, channel: null };
        case 'manual': return { knots: [['0', String(knotsAt(rmap, timeAt(rf, 0)) - timeAt(tf, 0) + BigInt(Math.round(method.seconds * 1e9)))]], residuals: [], quality: 1, note: null, channel: null };
        case 'sync': {
          const p = method.pairs.map(([a, b]) => [BigInt(a), BigInt(b)]);
          const knots = p.length === 1 ? [['0', String(p[0][1] - p[0][0])]] : [p[0], p[p.length - 1]].map(([a, b]) => [String(a), String(b)]);
          return { knots, residuals: p.map(([a]) => [String(a), 0]), quality: 1, note: null, channel: null };
        }
        default: throw 'No clear match';
      }
    },
    close({ file }) {
      fake.open.delete(file);
      return null;
    },
    async view({ file, generation, precision, entries }) {
      const o = fake.open.get(file);
      if (!o) throw 'that file is not open';
      const r = o.r;
      if (o.group) {
        const parts = entries.map((e) => {
          const traces = e.channels
            .filter((ch) => ch < r.chans.length)
            .map((ch) => {
              const { m, r: pr, local } = groupRoute(o, ch);
              const vp = { ...e, t0: String(knotsAt(m.knots, BigInt(e.t0), true)), t1: String(knotsAt(m.knots, BigInt(e.t1), true)) };
              const tr = gaps(pr, vp, reduce(pr, local, vp, pr.done || 0));
              return { channel: ch, coarse: false, fromPyramid: true, reduceUs: 5, ...tr, t: tr.t.map((t) => knotsAt(m.knots, BigInt(t))) };
            });
          return [e.id, encode(generation, e, null, traces)];
        });
        return encodeBatch(generation, parts);
      }
      const cold = !!fake.files[r.path]?.cold;
      const coarse = precision === 'coarse' && cold;
      fake.precisions.push(precision);
      for (const e of entries) fake.connectAsked.push(!!e.connectGaps);
      // a cold file's exact view reads the file: slow, as in the core
      if (cold && !coarse) await new Promise((ok) => setTimeout(ok, fake.files[r.path]?.exactMs ?? 800));
      const builtUntil = r.built ? null : r.done ? timeAt(r, r.done - 1) : BigInt(entries[0]?.t0 ?? 0);
      const parts = entries.map((e) => {
        const traces = e.channels
          .filter((ch) => ch < r.chans.length)
          .map((ch) => ({ channel: ch, coarse, fromPyramid: true, reduceUs: 5, ...gaps(r, e, reduce(r, ch, e, r.done || 0)) }));
        return [e.id, encode(generation, e, builtUntil, traces)];
      });
      return encodeBatch(generation, parts);
    },
    values_at({ file, channels, t }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      const og = fake.open.get(file);
      if (og.group) {
        const got = channels.filter((ch) => ch < r.chans.length).map((ch) => {
          const { m, r: pr, local } = groupRoute(og, ch);
          const own = knotsAt(m.knots, BigInt(t), true);
          const i = clampI(pr, Math.round(Number(own - (pr.t0 ?? T0)) / Number(dtNs(pr))));
          return { channel: ch, index: i, t: String(knotsAt(m.knots, timeAt(pr, i))), value: valueAt(pr, local, i) };
        });
        fake.lastValues = Object.fromEntries(got.map((n) => [n.channel, n.value]));
        return got;
      }
      const i = clampI(r, Math.round(Number(BigInt(t) - (r.t0 ?? T0)) / Number(dtNs(r))));
      const got = channels.filter((ch) => ch < r.chans.length).map((ch) => ({ channel: ch, index: i, t: String(timeAt(r, i)), value: valueAt(r, ch, i) }));
      fake.lastValues = Object.fromEntries(got.map((n) => [n.channel, n.value]));
      return got;
    },
    stats({ file, channels, span }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      if (!r.built) return null;
      const a = clampI(r, Math.ceil(Number(BigInt(span.t0) - (r.t0 ?? T0)) / Number(dtNs(r))));
      const b = clampI(r, Math.ceil(Number(BigInt(span.t1) - (r.t0 ?? T0)) / Number(dtNs(r))) - 1);
      return channels.map((ch) => {
        let [n, sum, sq, min, max, minI, maxI] = [0, 0, 0, Infinity, -Infinity, a, a];
        for (let i = a; i <= b; i++) {
          const v = valueAt(r, ch, i);
          n++;
          sum += v;
          sq += v * v;
          if (v < min) [min, minI] = [v, i];
          if (v > max) [max, maxI] = [v, i];
        }
        const mean = n ? sum / n : null;
        return { channel: ch, count: n, min: n ? min : null, minT: n ? String(timeAt(r, minI)) : null, max: n ? max : null, maxT: n ? String(timeAt(r, maxI)) : null, mean, std: n ? Math.sqrt(Math.max(0, sq / n - mean * mean)) : null };
      });
    },
    // the raw samples in [t0, t1), rows offset..offset + limit, in the app's rows format
    rows({ file, channels, span, offset, limit, query }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      if (query != null) {
        const ids = fake.queries.get(query);
        if (!ids) throw 'that query is gone';
        const win = ids.ids.slice(offset, offset + Math.min(limit, 10000));
        const chans = channels.filter((ch) => ch < r.chans.length);
        fake.rowsAsked.push(win.length);
        const n = win.length;
        const buf = new ArrayBuffer(32 + 8 * n * (2 + chans.length));
        const v = new DataView(buf);
        [...'TLMR'].forEach((c, i) => v.setUint8(i, c.charCodeAt(0)));
        v.setUint32(4, chans.length, true);
        v.setBigUint64(16, BigInt(ids.ids.length), true);
        v.setUint32(24, n, true);
        v.setUint32(28, 1, true);
        win.forEach((id, k) => {
          const i = ids.first + id;
          v.setBigInt64(32 + 8 * k, timeAt(r, i), true);
          v.setBigUint64(32 + 8 * n + 8 * k, BigInt(i), true);
          chans.forEach((ch, c) => v.setFloat64(32 + 16 * n + 8 * n * c + 8 * k, valueAt(r, ch, i), true));
        });
        return buf;
      }
      const lower = (t) => Math.max(0, Math.min(r.samples, Math.ceil(Number(BigInt(t) - (r.t0 ?? T0)) / Number(dtNs(r)))));
      const a = lower(span.t0);
      const b = Math.max(a, lower(span.t1));
      const start = Math.min(b, a + offset);
      const n = Math.min(b - start, limit, 10000);
      const chans = channels.filter((ch) => ch < r.chans.length);
      fake.rowsAsked.push(n);
      const buf = new ArrayBuffer(32 + 8 * n * (1 + chans.length));
      const v = new DataView(buf);
      [...'TLMR'].forEach((c, i) => v.setUint8(i, c.charCodeAt(0)));
      v.setUint32(4, chans.length, true);
      v.setBigUint64(8, BigInt(start), true);
      v.setBigUint64(16, BigInt(b - a), true);
      v.setUint32(24, n, true);
      for (let k = 0; k < n; k++) v.setBigInt64(32 + 8 * k, timeAt(r, start + k), true);
      chans.forEach((ch, c) => {
        for (let k = 0; k < n; k++) v.setFloat64(32 + 8 * n * (c + 1) + 8 * k, valueAt(r, ch, start + k), true);
      });
      return buf;
    },
    // table sort and filter: the same language as the core's, over the fake's samples
    filter_check: ({ expr }) => (parseFilter(expr).error ?? null),
    table_cancel() {},
    table_hit({ file, token, k }) {
      const q = fake.queries.get(token);
      const h = q?.hits[k];
      if (!h) return null;
      const r = fake.open.get(file).r;
      const row = Math.max(0, Number((h[0] - (r.t0 ?? T0)) / dtNs(r)) - q.first);
      const asc = [...q.ids].sort((x, y) => x - y);
      const first = asc.find((id) => id >= row) ?? row;
      return { t0: String(h[0]), t1: String(h[1]), pos: q.ids.indexOf(first) };
    },
    // Apply to charts: the rows the filters don't pass are left out; the fake only counts them
    table_mask({ file, owner, filters }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      fake.maskCalls.push({ owner, filters });
      const preds = filters.map((f) => [f.col, parseFilter(f.expr).test]);
      const val = (col, i) => (col === 't' ? Number(timeAt(r, i)) : valueAt(r, Number(col.slice(1)), i));
      let kept = 0;
      const runs = [];
      for (let i = 0; i < r.samples; i++) {
        if (preds.every(([c, t]) => t(val(c, i)))) kept++;
        else if (runs.length && runs.at(-1)[2] === i - 1) [runs.at(-1)[1], runs.at(-1)[2]] = [timeAt(r, i), i];
        else runs.push([timeAt(r, i), timeAt(r, i), i]);
      }
      fake.masks.set(owner, r.samples - kept);
      fake.maskRuns.set(owner, runs.map(([a, b]) => [a, b]));
      return { matched: kept, total: r.samples, done: true, hits: 0 };
    },
    // what the charts mark: the query's matches and what the masks leave out, in the span
    table_marks({ token, t0, t1, widthPx }) {
      fake.marksAsked.push({ token, t0, t1, widthPx });
      const [a, b] = [BigInt(t0), BigInt(t1)];
      const within = (runs) => runs.filter(([x, y]) => y >= a && x < b).map(([x, y]) => [String(x), String(y)]);
      return { hits: within(fake.queries.get(token)?.hits ?? []), excluded: within([...fake.maskRuns.values()].flat()) };
    },
    table_unmask({ owner }) {
      fake.masks.delete(owner);
      fake.maskRuns.delete(owner);
    },
    table_query({ file, span, filters, sort, token, dropToken }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      fake.queries.delete(dropToken);
      fake.queriesRun.push({ filters, sort, span });
      const lower = (t) => Math.max(0, Math.min(r.samples, Math.ceil(Number(BigInt(t) - (r.t0 ?? T0)) / Number(dtNs(r)))));
      const a = lower(span.t0);
      const b = Math.max(a, lower(span.t1));
      const val = (col, i) => (col === 't' ? Number(timeAt(r, i)) : valueAt(r, Number(col.slice(1)), i));
      const preds = filters.map((f) => [f.col, parseFilter(f.expr).test]);
      let ids = [];
      for (let i = a; i < b; i++) if (preds.every(([c, t]) => t(val(c, i)))) ids.push(i - a);
      const cmp = (x, y) => {
        for (const k of sort) {
          const [p, q] = [val(k.col, a + x), val(k.col, a + y)];
          if (Number.isNaN(p) || Number.isNaN(q)) { if (Number.isNaN(p) !== Number.isNaN(q)) return Number.isNaN(p) ? 1 : -1; continue; }
          if (p !== q) return (p < q ? -1 : 1) * (k.desc ? -1 : 1);
        }
        return x - y;
      };
      if (sort.length) ids.sort(cmp);
      // matches as runs of time, those closer than 0.25 s joined
      const hits = [];
      if (preds.length) {
        for (const id of [...ids].sort((x, y) => x - y)) {
          const t = timeAt(r, a + id);
          const last = hits.at(-1);
          if (last && t - last[1] <= 250000000n) last[1] = t;
          else hits.push([t, t]);
        }
      }
      fake.queries.set(token, { first: a, ids, hits });
      return { matched: ids.length, total: b - a, done: true, hits: hits.length };
    },
    // a live source: a recording that grows with the clock at its rate, until stopped. A replay's pause stops its clock;
    // a device's doesn't (the recording goes on, with a gap).
    live_connect({ args }) {
      const { endpoint, descriptor, name, record, dir, addTo } = args;
      fake.liveConnects.push(JSON.parse(JSON.stringify(args)));
      const replay = endpoint.startsWith('replay://');
      const spec = fake.sources[endpoint] ?? (replay ? { rate: 1000, already: 1 } : null);
      if (!spec) throw `couldn't connect to ${endpoint}: connection refused`;
      if (!replay && !endpoint.startsWith('file://') && !descriptor) throw `${endpoint} needs a descriptor for its frames`;
      const r = recording(endpoint, { ...spec, samples: 1 });
      r.base = r.chans.slice();
      const t0 = Date.now() - (spec.already ?? 1) * 1000;
      let stopped = null;
      let pausedAt = null;
      let pausedFor = 0;
      let paused = false;
      // the clock the recording grows with: a replay's stops while paused
      const clock = () => (stopped ?? (replay && pausedAt != null ? pausedAt : Date.now())) - (replay ? pausedFor : 0);
      const n = () => Math.max(1, Math.floor(((clock() - t0) / 1000) * r.rate));
      Object.defineProperty(r, 'samples', { get: n });
      Object.defineProperty(r, 'done', { get: n });
      const id = nextFile++;
      const health = () => ({ endpoint, bytes_in: n() * 24, records_in: n(), records_dropped: fake.liveDropped ?? 0, records_written: n(), sequence_missing: 0, sequence_repeated: 0, malformed_bytes: 0, rows_skipped: 0, rows_back_in_time: 0, files: [`/rec/session-1/source-${addTo != null ? 2 : 1}.tlm`], ring_bytes: 1 << 26, ring_high_water_bytes: 4096, ended: stopped != null, error: null, paused, seconds: (clock() - t0) / 1000 });
      const live = {
        health,
        name: name || endpoint,
        recording: record !== false,
        stopped: () => stopped != null,
        stop: () => (stopped ??= Date.now()),
        pause: (p) => {
          if (p === paused) return;
          paused = p;
          if (p) pausedAt = Date.now();
          else {
            pausedFor += Date.now() - pausedAt;
            pausedAt = null;
          }
        },
        trigger: null,
      };
      fake.open.set(id, { r, progress: null, live });
      fake.files[endpoint] = spec;
      const info = () => ({ path: endpoint, format: 'tlm', samples: n(), time: { kind: 'rate', hz: r.rate }, channels: r.chans, skipped: [] });
      fake.open.get(id).info = info;
      return { id, info: info(), range: [String(timeAt(r, 0)), String(timeAt(r, n() - 1))], status: { state: 'built' } };
    },
    live_state({ file }) {
      const o = fake.open.get(file);
      if (!o?.live) throw 'that source is not open';
      const L = o.live;
      const last = timeAt(o.r, o.r.samples - 1);
      // a trigger fires `fake.triggerAfter` ms after it was armed (and again that long after, when it repeats)
      const T = L.trigger;
      if (T?.armed && Date.now() >= T.nextAt) {
        T.count++;
        T.last = String(last);
        T.armed = T.spec.repeat;
        T.nextAt = Date.now() + (fake.triggerAfter ?? 1000);
        const ev = { id: `trigger:${last}`, name: `Trigger: ${T.spec.channel} ${{ rise: 'rises through', fall: 'falls through', above: 'goes above', below: 'goes below' }[T.spec.kind]} ${T.spec.level}`, start: String(last), duration: '0', category: 'trigger', source: 'rule', channels: [T.spec.channel] };
        fake.events[o.r.path] = [...(fake.events[o.r.path] ?? []), ev];
      }
      const trigger = T ? { armed: T.armed, count: T.count, last: T.last, pending: T.last != null && BigInt(T.last) + BigInt(Math.round(T.spec.post * 1e9)) > last, error: null, spec: T.spec, rule: `prev(${T.spec.channel}) < ${T.spec.level} && ${T.spec.channel} >= ${T.spec.level}` } : { armed: false, count: 0, last: null, pending: false, error: null, spec: null, rule: null };
      return { range: [String(timeAt(o.r, 0)), String(last)], samples: o.r.samples, part: 1, info: o.info(), session: '/rec/session-1', name: L.name, recording: L.recording, stopped: L.stopped(), health: L.health(), trigger };
    },
    live_stop({ file }) {
      const o = fake.open.get(file);
      o.live.stop();
      return o.live.health();
    },
    live_pause({ file, paused }) {
      fake.open.get(file).live.pause(paused);
      return null;
    },
    live_trigger({ file, spec }) {
      const L = fake.open.get(file).live;
      const chans = fake.open.get(file).r.chans.map((c) => c.name);
      if (!spec) {
        if (L.trigger) L.trigger.armed = false;
      } else {
        if (/cumsum/.test(spec.channel)) throw "This channel needs the whole recording to compute, so it can't trigger live";
        if (!chans.includes(spec.channel) && !/[(+\-*/]/.test(spec.channel)) throw `There's no channel called ${spec.channel}`;
        fake.triggers.push(JSON.parse(JSON.stringify(spec)));
        L.trigger = { spec, armed: true, count: 0, last: null, nextAt: Date.now() + (fake.triggerAfter ?? 1000) };
      }
      const T = L.trigger;
      return T ? { armed: T.armed, count: T.count, last: T.last, pending: false, error: null, spec: T.spec, rule: `prev(${T.spec.channel}) < ${T.spec.level} && ${T.spec.channel} >= ${T.spec.level}` } : { armed: false, count: 0, last: null, pending: false, error: null, spec: null, rule: null };
    },
    live_preview({ endpoint, descriptor }) {
      const path = endpoint.replace(/^(replay|file):\/\//, '').replace(/\?.*$/, '');
      if (/^(replay|file):/.test(endpoint)) {
        const r = recording(path, fake.files[path] ?? {});
        return { detail: `${path.endsWith('.csv') ? 'CSV' : 'tlm'}, ${r.samples.toLocaleString()} samples`, channels: r.chans.map((c) => [c.name, c.unit]) };
      }
      if (!descriptor) throw "Choose the frames' descriptor to see how they decode";
      const spec = fake.sources[endpoint] ?? { channels: ['rpm', 'temp'] };
      return { detail: '16 bytes a frame, 1000 Hz', channels: spec.channels.map((c) => [c, null]) };
    },
    'plugin:dialog|open': ({ options }) => (options?.directory ? fake.pickFolder : fake.pick),
    // ask() is a confirm message: it answers with the label of the button pressed
    'plugin:dialog|message': ({ message, buttons }) => ((fake.asked = message), (fake.answer ?? true) ? (buttons?.OkCancelCustom?.[0] ?? 'Yes') : (buttons?.OkCancelCustom?.[1] ?? 'No')),
    'plugin:dialog|save': () => fake.savePick,
    config_use({ folder }) {
      fake.cfgCalls.push({ cmd: 'use', folder });
      return { dir: folder ? `${folder}/.tlmbench` : null, from: fake.cfgFrom, recipes: fake.cfgRecipes, checks: fake.cfgChecks, problems: [] };
    },
    config_scan({ folder, own }) {
      fake.cfgCalls.push({ cmd: 'scan', folder, own });
      return { rows: fake.cfgRows, problems: fake.cfgProblems, writable: fake.cfgWritable, git: fake.cfgGit };
    },
    config_mirror({ own }) {
      fake.cfgCalls.push({ cmd: 'mirror', own });
      return null;
    },
    config_share({ folder, own, keys }) {
      fake.cfgCalls.push({ cmd: 'share', folder, own, keys });
      if (fake.readOnly.includes(folder)) throw `can't write to ${folder}/.tlmbench: Read-only file system (os error 30)`;
      fake.cfgRows = fake.cfgRows.map((r) => (keys.includes(`${r.kind}:${r.key}`) ? { ...r, state: 'same', detail: null } : r));
      return keys.map((k) => k.slice(k.indexOf(':') + 1));
    },
    config_pull({ folder, own, keys }) {
      fake.cfgCalls.push({ cmd: 'pull', folder, own, keys });
      fake.cfgRows = fake.cfgRows.map((r) => (keys.includes(`${r.kind}:${r.key}`) ? { ...r, state: 'same', detail: null } : r));
      return { ...fake.cfgPull, taken: keys };
    },
    config_export({ out, own }) {
      fake.cfgCalls.push({ cmd: 'export', out, own });
      return ['recipes/power.toml', 'categories.toml'];
    },
    config_gitignore({ folder, lines }) {
      fake.cfgCalls.push({ cmd: 'gitignore', folder, lines });
      return lines;
    },
    timemap_save({ map }) {
      fake.timemaps = fake.timemaps.filter((m) => !(m.primary === map.primary && m.file === map.file)).concat(map);
      return null;
    },
    timemap_find({ primary, file }) {
      const p = fake.open.get(primary)?.r.path;
      const f = fake.open.get(file)?.r.path;
      return fake.timemapsByPath?.[`${p}|${f}`] ?? null;
    },
    // shared views: `views` is folder → file name → the view (a string: a file that isn't JSON)
    shared_views({ folder }) {
      const vs = fake.views[folder] ?? {};
      return Object.keys(vs)
        .sort()
        .map((file) => (typeof vs[file] === 'string' ? { file, view: null, error: 'expected value at line 1 column 1' } : { file, view: vs[file], error: null }));
    },
    share_view({ folder, file, view }) {
      if (fake.readOnly.includes(folder)) throw `can't write to ${folder}/.tlmbench/views: Read-only file system (os error 30)`;
      (fake.views[folder] ??= {})[file] = { ...view, by: 'tester' };
      return `${folder}/.tlmbench/views/${file}`;
    },
    // Export data…: what the page asked the core to write
    export_data({ req }) {
      fake.dataExports.push(req);
      if (fake.exportFails) throw fake.exportFails;
      return { path: req.out, channelsFile: req.out.replace(/\.[^.]+$/, '.channels.csv'), rows: 1234, bytes: 99999 };
    },
    save_file({ path, bytes }) {
      fake.saved_files.push({ path, bytes });
      return null;
    },
    export_view({ path, view }) {
      fake.exported.push({ path, view });
      return null;
    },
    list_folder({ path }) {
      // a folder's tree from the paths of `fake.files` under it (the core lists the disk)
      const files = [...new Set([...Object.keys(fake.files), ...Object.keys(fake.bins)])].filter((p) => p.startsWith(path.replace(/\/$/, '') + '/'));
      if (!files.length && !fake.folders.includes(path)) throw `${path} isn't a folder that can be read`;
      const root = { path, name: path.split('/').pop(), dir: true, bytes: 0, children: [] };
      for (const f of [...files, ...(fake.unopenable ?? []).filter((p) => p.startsWith(path.replace(/\/$/, '') + '/'))].sort()) {
        let node = root;
        const parts = f.slice(path.length + 1).split('/');
        parts.forEach((part, k) => {
          const p = node.path + '/' + part;
          let child = node.children.find((c) => c.path === p);
          if (!child) {
            child = { path: p, name: part, dir: k < parts.length - 1, bytes: k < parts.length - 1 ? 0 : 1234567, openable: !(fake.unopenable ?? []).includes(p), ...(fake.bins[p] && k === parts.length - 1 && !claimed(p) ? { needsDescriptor: true } : {}), children: [] };
            node.children.push(child);
          }
          node = child;
        });
      }
      const sort = (n) => {
        n.children.sort((a, b) => b.dir - a.dir || a.name.localeCompare(b.name));
        n.children.forEach(sort);
      };
      sort(root);
      return root;
    },
    'plugin:event|listen'({ event, handler }) {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(handler);
      return handler;
    },
    'plugin:event|unlisten': () => null,
    // the cache: `cache` is its entries (bytes each; open: in use, kept by Clear)
    cache_info: () => ({ path: '/Users/me/Library/Caches/tlmbench', bytes: fake.cache.reduce((a, e) => a + e.bytes, 0), cap: 20 * 2 ** 30 }),
    cache_reveal() {
      fake.revealed = (fake.revealed || 0) + 1;
    },
    cache_clear() {
      const gone = fake.cache.filter((e) => !e.open);
      fake.cache = fake.cache.filter((e) => e.open);
      return gone.length;
    },
    // the core's memory and CPU time: the CPU time grows by a quarter second a second (a load of 25%)
    usage() {
      const now = performance.now() / 1000;
      return { memory: 412 * 2 ** 20, chunks: 96 * 2 ** 20, chunkBudget: 2 * 2 ** 30, cpuSeconds: now * 0.25 };
    },
    // the settings every window shares (null before the first save); a save reaches the other windows as an event
    settings_get: () => fake.sharedSettings ?? null,
    settings_save({ text }) {
      fake.sharedSettings = text;
      return null;
    },
    // windows: a new one is recorded (with what it opens), the first asks for nothing
    window_new({ handoff, frame, hold }) {
      const label = `win-${fake.windows.length + 2}`;
      fake.windows.push({ label, handoff, frame, hold });
      return label;
    },
    holders: () => fake.holdersOf ?? [],
    window_take: () => fake.handoff ?? null,
    window_frame: () => fake.frame ?? { x: 10, y: 20, w: 1200, h: 760 },
    window_place({ frame }) {
      fake.placed = frame;
      return null;
    },
    'plugin:window|set_title'({ value }) {
      fake.title = value;
      return null;
    },
  };

  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    async invoke(cmd, args) {
      fake.calls.push(cmd);
      const f = commands[cmd];
      if (!f) throw `the fake core has no command ${cmd}`;
      await new Promise((ok) => setTimeout(ok, 1));
      return f(args ?? {});
    },
    transformCallback(cb, once = false) {
      const id = nextCb++;
      callbacks.set(id, (data) => {
        if (once) callbacks.delete(id);
        return cb?.(data);
      });
      return id;
    },
    unregisterCallback: (id) => callbacks.delete(id),
    runCallback: (id, data) => callbacks.get(id)?.(data),
    convertFileSrc: (p) => p,
  };
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
})();
