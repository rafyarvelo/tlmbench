// A fake of the app's core for the UI tests: it answers the Tauri commands the UI calls (launch, open, close,
// view, values_at, stats, the dialog and the drag-and-drop events) from synthetic recordings, and encodes view
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
  const TRACE_HEADER = 24;
  const BATCH_HEADER = 16;
  const BATCH_ENTRY = 16;
  const up8 = (n) => (n + 7) & ~7;

  const bucketNs = (vp) => {
    const b = (BigInt(vp.t1) - BigInt(vp.t0)) / BigInt(Math.max(1, vp.widthPx));
    return b < 1n ? 1n : b;
  };

  /** One reply: `traces` are { channel, kind: 'raw'|'m4', coarse, fromPyramid, reduceUs, t: bigint[], y: number[] }. */
  function encode(generation, vp, builtUntil, traces) {
    const size = HEADER + traces.reduce((a, t) => a + TRACE_HEADER + 8 * t.t.length, 0);
    const buf = new ArrayBuffer(size);
    const v = new DataView(buf);
    const t0 = BigInt(vp.t0);
    const t1 = BigInt(vp.t1);
    const scale = Number(bucketNs(vp));
    [...'TLMV'].forEach((c, i) => v.setUint8(i, c.charCodeAt(0)));
    v.setUint16(4, 2, true);
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
      const d = o + TRACE_HEADER;
      t.t.forEach((ts, i) => v.setFloat32(d + 4 * i, Number(ts - t0) / scale, true));
      t.y.forEach((y, i) => v.setFloat32(d + 4 * n + 4 * i, y, true));
      o += TRACE_HEADER + 8 * n;
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
  /** A recording: `rate` Hz for `samples` samples; channel k is a few sines, with NaNs where `gaps` says. */
  function recording(path, { channels = ['accel_x', 'accel_y', 'roll', 'pitch', 'motor_rpm', 'motor_rpm_target'], units, samples = 200000, rate = 1000, built = true } = {}) {
    const chans = channels.map((name, k) => ({ name, unit: units?.[k] ?? (name.includes('rpm') ? 'rpm' : name.startsWith('accel') ? 'g' : 'deg'), dtype: 'f32' }));
    return { path, chans, samples, rate, built, done: built ? samples : 0, progress: null };
  }
  const dtNs = (r) => BigInt(Math.round(1e9 / r.rate));
  const timeAt = (r, i) => T0 + BigInt(i) * dtNs(r);
  const valueAt = (r, ch, i) => {
    const s = i / r.rate;
    return Math.fround((ch + 1) * Math.sin(s * (0.05 + 0.031 * ch)) + 0.3 * Math.sin(s * (1.7 + ch)));
  };
  const indexAt = (r, t) => Number((BigInt(t) - T0) / dtNs(r));
  const clampI = (r, i) => Math.max(0, Math.min(r.samples - 1, i));

  /** One channel over a viewport: raw samples (and one neighbour each side) up to 4W, else M4 per pixel column. */
  function reduce(r, ch, vp, upTo) {
    const lim = Math.min(r.samples, upTo);
    const a = clampI(r, indexAt(r, vp.t0));
    const b = Math.min(lim - 1, Math.max(a, Math.ceil(Number(BigInt(vp.t1) - T0) / Number(dtNs(r)))));
    const t = [];
    const y = [];
    if (lim <= 0 || a >= lim) return { kind: 'raw', t, y };
    if (b - a + 1 <= 4 * vp.widthPx) {
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

  // ---------- IPC ----------

  const fake = {
    files: {},
    launch: null,
    pick: null,
    /** What Open folder returns, and folders that exist without recordings. */
    pickFolder: null,
    folders: [],
    calls: [],
    /** The precision each `view` asked for. */
    precisions: [],
    open: new Map(),
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
    drop(paths) {
      for (const ev of ['tauri://drag-enter', 'tauri://drag-drop']) emit(ev, { paths, position: { x: 200, y: 200 } });
    },
  };
  window.__tlmFake = fake;

  const callbacks = new Map();
  let nextCb = 1;
  const listeners = new Map();
  const send = (ch, message) => callbacks.get(ch.id)?.({ message, index: ch.next++ });
  const emit = (event, payload) => (listeners.get(event) || []).forEach((h) => callbacks.get(h)?.({ event, id: h, payload }));

  let nextFile = 1;
  const commands = {
    launch: () => ({ file: fake.launch, gate: null, gateMode: null }),
    open({ path, progress }) {
      const spec = fake.files[path];
      if (!spec) throw `couldn't open ${path}: no such file`;
      const r = recording(path, spec);
      const id = nextFile++;
      const ch = progress && { id: progress.id ?? Number(String(progress).replace(/\D/g, '')), next: 0 };
      fake.open.set(id, { r, progress: ch });
      const info = { path, format: path.endsWith('.csv') ? 'csv' : 'tlm', samples: r.samples, time: { kind: 'rate', hz: r.rate }, channels: r.chans, skipped: [] };
      return { id, info, range: [String(timeAt(r, 0)), String(timeAt(r, r.samples - 1))], status: r.built ? { state: 'built' } : { state: 'building', done: 0, total: r.samples } };
    },
    close({ file }) {
      fake.open.delete(file);
      return null;
    },
    async view({ file, generation, precision, entries }) {
      const o = fake.open.get(file);
      if (!o) throw 'that file is not open';
      const r = o.r;
      const cold = !!fake.files[r.path].cold;
      const coarse = precision === 'coarse' && cold;
      fake.precisions.push(precision);
      // a cold file's exact view reads the file: slow, as in the core
      if (cold && !coarse) await new Promise((ok) => setTimeout(ok, fake.files[r.path].exactMs ?? 800));
      const builtUntil = r.built ? null : r.done ? timeAt(r, r.done - 1) : BigInt(entries[0]?.t0 ?? 0);
      const parts = entries.map((e) => {
        const traces = e.channels
          .filter((ch) => ch < r.chans.length)
          .map((ch) => ({ channel: ch, coarse, fromPyramid: true, reduceUs: 5, ...reduce(r, ch, e, r.done || 0) }));
        return [e.id, encode(generation, e, builtUntil, traces)];
      });
      return encodeBatch(generation, parts);
    },
    values_at({ file, channels, t }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      const i = clampI(r, Math.round(Number(BigInt(t) - T0) / Number(dtNs(r))));
      return channels.filter((ch) => ch < r.chans.length).map((ch) => ({ channel: ch, index: i, t: String(timeAt(r, i)), value: valueAt(r, ch, i) }));
    },
    stats({ file, channels, span }) {
      const r = fake.open.get(file)?.r;
      if (!r) throw 'that file is not open';
      if (!r.built) return null;
      const a = clampI(r, Math.ceil(Number(BigInt(span.t0) - T0) / Number(dtNs(r))));
      const b = clampI(r, Math.ceil(Number(BigInt(span.t1) - T0) / Number(dtNs(r))) - 1);
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
    'plugin:dialog|open': ({ options }) => (options?.directory ? fake.pickFolder : fake.pick),
    list_folder({ path }) {
      // a folder's tree from the paths of `fake.files` under it (the core lists the disk)
      const files = Object.keys(fake.files).filter((p) => p.startsWith(path.replace(/\/$/, '') + '/'));
      if (!files.length && !fake.folders.includes(path)) throw `${path} isn't a folder that can be read`;
      const root = { path, name: path.split('/').pop(), dir: true, bytes: 0, children: [] };
      for (const f of files.sort()) {
        let node = root;
        const parts = f.slice(path.length + 1).split('/');
        parts.forEach((part, k) => {
          const p = node.path + '/' + part;
          let child = node.children.find((c) => c.path === p);
          if (!child) {
            child = { path: p, name: part, dir: k < parts.length - 1, bytes: k < parts.length - 1 ? 0 : 1234567, children: [] };
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
