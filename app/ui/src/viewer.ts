// The viewer: the UI over an export folder instead of the app's core. The app's commands are answered by the core
// compiled to WASM (crates/web) over the data the export holds, fetched as static files: `export.json`, then each
// file's times, pyramids and raw chunks (design: Export to HTML). Installed before anything calls `invoke`.
//
// A viewer opens its files itself (the export names them), shows the layout it was exported with, and can't import,
// edit layouts, rules or checks, or make derived channels.

type Handler = (data: unknown) => void;

interface ExportChannel {
  index: number;
  name: string;
  raw: boolean;
  raw_chunks: number;
  pyramid: { first_level: number; levels: number[] };
}
interface ExportFile {
  k: number;
  name: string;
  path: string;
  samples: number;
  chunk: number;
  time: { kind: 'rate' | 'column'; chunks?: number };
  channels: ExportChannel[];
}
export interface Manifest {
  format: string;
  version: number;
  report: { mode: 'full' | 'light'; trimmed: boolean; bytes: number; budget: number };
  files: ExportFile[];
  layout: unknown;
  events: { file: number; events: unknown[] }[];
  checks: { file: number; set: { name: string }; result: unknown; events: unknown[]; t_first: string }[];
}

/** What the page knows of the export, for the shell (title, the checks summary, the mode label). */
export const viewer = { manifest: null as Manifest | null };

const enc = new TextEncoder();
const dec = new TextDecoder();

export const EXPORT_PREFIX = 'export://f';

/** The core compiled to WASM behind one `call`. */
async function loadCore(url: string) {
  const res = await fetch(url);
  const { instance } = await WebAssembly.instantiateStreaming(res, {});
  const x = instance.exports as unknown as {
    memory: WebAssembly.Memory;
    alloc(n: number): number;
    dealloc(p: number, n: number): void;
    call(c: number, cl: number, a: number, al: number, b: number, bl: number): number;
    reply_len(): number;
  };
  const put = (b: Uint8Array) => {
    const p = x.alloc(b.length);
    new Uint8Array(x.memory.buffer, p, b.length).set(b);
    return p;
  };
  /** Runs a command: JSON replies parse, binary ones come back as an ArrayBuffer, errors throw. */
  return function call(cmd: string, args: unknown = null, bin: Uint8Array = new Uint8Array(0)): unknown {
    const c = enc.encode(cmd);
    const a = args === null ? new Uint8Array(0) : enc.encode(JSON.stringify(args));
    const [pc, pa, pb] = [put(c), put(a), put(bin)];
    const p = x.call(pc, c.length, pa, a.length, pb, bin.length);
    const out = new Uint8Array(x.memory.buffer, p, x.reply_len()).slice();
    x.dealloc(pc, c.length);
    x.dealloc(pa, a.length);
    x.dealloc(pb, bin.length);
    const body = out.subarray(1);
    if (out[0] === 2) throw new Error(dec.decode(body));
    return out[0] === 0 ? JSON.parse(dec.decode(body)) : body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength);
  };
}

export async function installViewerCore(): Promise<void> {
  const base = new URL('.', document.baseURI).href;
  const get = async (path: string) => {
    const r = await fetch(new URL(path, base));
    if (!r.ok) throw new Error(`${path}: ${r.status}`);
    return r;
  };
  const manifest = (await (await get('export.json')).json()) as Manifest;
  viewer.manifest = manifest;
  const core = await loadCore(new URL('tlmbench_web.wasm', base).href);
  const bytes = async (path: string) => new Uint8Array(await (await get(path)).arrayBuffer());

  const callbacks = new Map<number, Handler>();
  let nextCb = 1;

  /** Loads file k: times and pyramids first (the view opens on them), then the raw chunks, reporting progress. */
  async function openFile(k: number, progress: Handler | null) {
    const f = manifest.files[k];
    if (!f) throw new Error('that file is not in this export');
    const opened0 = core('open_export', { file: f }) as { status: unknown };
    void opened0;
    const dir = `data/f${k}/`;
    if (f.time.kind === 'column') {
      const parts = await Promise.all(Array.from({ length: f.time.chunks ?? 0 }, (_, j) => bytes(`${dir}t.${j}.bin`)));
      for (const p of parts) core('put_times', { file: k }, p);
    }
    await Promise.all(
      f.channels.map(async (c, i) => {
        const pyr = await bytes(`${dir}c${c.index}.pyr.bin`);
        core('put_pyr', { file: k, chan: i, levels: c.pyramid.levels, first_level: c.pyramid.first_level }, pyr);
      }),
    );
    const opened = core('opened', { file: k }) as Record<string, unknown>;
    const wanted = f.channels.flatMap((c, i) => Array.from({ length: c.raw ? c.raw_chunks : 0 }, (_, j) => ({ i, j, c })));
    const total = f.samples * f.channels.filter((c) => c.raw).length;
    if (!wanted.length) return opened;
    // the raw chunks arrive in the background; the view draws from the pyramid until a channel is whole
    const send = (done: number, error: string | null = null) => (location.search.includes('debug') && console.log('progress', done, total, error), progress?.({ message: { done, total, builtUntil: null, error }, index: sent++ }));
    let sent = 0;
    // after the reply has been taken: a progress message that comes first would be overwritten by it
    setTimeout(() => void (async () => {
      let done = 0;
      try {
        const per = f.channels.map(() => [] as Promise<Uint8Array>[]);
        for (const w of wanted) per[w.i][w.j] = bytes(`${dir}c${w.c.index}.${w.j}.bin`);
        for (let i = 0; i < per.length; i++) {
          for (const p of per[i]) {
            const b = await p;
            core('put_raw', { file: k, chan: i, chunk: 0 }, b);
            done += b.length / 8;
            send(done);
          }
        }
        send(total);
        (window as unknown as { __tlmViewerLoaded?: boolean }).__tlmViewerLoaded = true;
      } catch (e) {
        send(0, String(e));
      }
    })(), 30);
    return { ...opened, status: { state: 'building', done: 0, total } };
  }

  const none = () => null;
  const commands: Record<string, (a: Record<string, unknown>) => unknown | Promise<unknown>> = {
    'plugin:event|listen': (a) => a.handler,
    'plugin:event|unlisten': none,
    'plugin:window|set_title': (a) => ((document.title = String(a.value)), null),
    launch: () => ({ file: null, gate: null, gateMode: null, gateSeconds: null }),
    // what the page does on its own once the app is up (see app.ts, `viewerBoot`)
    viewer_boot: () => ({ files: manifest.files.map((f) => `${EXPORT_PREFIX}${f.k}`), layout: manifest.layout, events: manifest.events, checks: manifest.checks, report: manifest.report }),
    open: (a) => {
      const path = String(a.path);
      if (!path.startsWith(EXPORT_PREFIX)) throw new Error('A viewer opens only what its export holds.');
      const ch = a.progress as { id?: number } | string | undefined;
      const id = typeof ch === 'string' ? Number(ch.replace('__CHANNEL__:', '')) : ch?.id;
      return openFile(Number(path.slice(EXPORT_PREFIX.length)), id != null ? (callbacks.get(id) ?? null) : null);
    },
    close: (a) => core('close', { file: a.file }),
    view: (a) => {
      const r = core('view', a);
      // Gate 5 measures the open from the navigation to the first chart view
      (window as unknown as { __tlmFirstView?: number }).__tlmFirstView ??= performance.now();
      return r;
    },
    values_at: (a) => core('values_at', a),
    stats: (a) => core('stats', a),
    rows: (a) => core('rows', a),
    settings_get: none,
    settings_save: none,
    set_expr_env: none,
    hold: none,
    holders: () => [],
    usage: () => ({ used: 0, budget: 0 }),
    events_list: (a) => (manifest.events.find((e) => e.file === Number(a.file) - 0)?.events ?? []),
    // the check sets the export ran, and their results as the app's Checks pane reads them
    check_sets_list: () => [...new Map(manifest.checks.map((c) => [c.set.name, { ...c.set, runs_on_any_file: true }])).values()],
    checks_run: (a) => manifest.checks.filter((c) => c.file === Number(a.file) && (a.sets as string[]).includes(c.set.name)).map((c) => ({ set: c.set.name, result: c.result, events: c.events, t_first: c.t_first })),
    recipes_list: () => [],
    plugins_list: () => [],
    shared_views: () => [],
    needs_descriptor: () => false,
    packet_parts: () => [],
    window_frame: none,
    window_place: none,
    window_at: none,
    window_take: none,
    group_state: none,
    missing_between: () => [],
    filter_check: none,
    browser_access: () => ({ on: false }),
  };

  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    invoke: async (cmd: string, args?: Record<string, unknown>) => {
      if (location.search.includes('debug')) console.log('invoke', cmd, JSON.stringify(args ?? {}).slice(0, 200));
      // a real IPC reply comes on a task of its own: answering in microtasks alone would starve timers and fetches
      await new Promise<void>((r) => setTimeout(r, 0));
      const h = commands[cmd];
      if (!h) throw new Error(`${cmd} is not available in an export`);
      return h(args ?? {});
    },
    transformCallback(cb: Handler, once = false) {
      const id = nextCb++;
      callbacks.set(id, (d) => {
        if (once) callbacks.delete(id);
        return cb(d);
      });
      return id;
    },
    unregisterCallback: (id: number) => callbacks.delete(id),
    runCallback: (id: number, d: unknown) => callbacks.get(id)?.(d),
    convertFileSrc: (p: string) => p,
  };
  (window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  (window as unknown as Record<string, unknown>).__tlmViewer = true;
}
