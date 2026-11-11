// The viewer: the UI over an export folder instead of the app's core. The app's commands are answered by the core
// compiled to WASM (crates/web) over the data the export holds, fetched as static files: `export.json`, then each
// file's times, pyramids and raw chunks (design: Export to HTML). Installed before anything calls `invoke`.
//
// A viewer opens its files itself (the export names them), shows the layout it was exported with, and can't import,
// edit layouts, rules or checks, or make derived channels.

import { type Commands, installInternals, loadCore, stubs } from './wasmcore';

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

export const EXPORT_PREFIX = 'export://f';

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

  // the internals are installed below; the channel callbacks `open` reports progress through live there
  let callbacks: Map<number, (d: unknown) => void> = new Map();

  /** Per file, per channel: resolves once its raw samples are in the core (at once for a channel that keeps none). */
  const rawReady = new Map<number, Promise<void>[]>();
  const waitRaw = async (file: unknown, channels: unknown) => {
    const ps = rawReady.get(Number(file)) ?? [];
    await Promise.all((channels as number[]).map((c) => ps[c]));
  };

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
    // what reads a channel's raw samples (a table, a value at a cursor, exact stats) waits for them: a missing sample is never
    // shown for one that is merely not here yet
    const gates = f.channels.map(() => {
      let ok!: () => void;
      let no!: (e: unknown) => void;
      const p = new Promise<void>((res, rej) => ((ok = res), (no = rej)));
      p.catch(() => {});
      return { p, ok, no };
    });
    rawReady.set(k, gates.map((g, i) => (f.channels[i].raw ? g.p : Promise.resolve())));
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
          gates[i].ok();
        }
        send(total);
        (window as unknown as { __tlmViewerLoaded?: boolean }).__tlmViewerLoaded = true;
      } catch (e) {
        gates.forEach((g) => g.no(e));
        send(0, String(e));
      }
    })(), 30);
    return { ...opened, status: { state: 'building', done: 0, total } };
  }

  const commands: Commands = {
    ...stubs,
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
    values_at: async (a) => (await waitRaw(a.file, a.channels), core('values_at', a)),
    stats: async (a) => (await waitRaw(a.file, a.channels), core('stats', a)),
    rows: async (a) => (await waitRaw(a.file, a.channels), core('rows', a)),
    events_list: (a) => (manifest.events.find((e) => e.file === Number(a.file) - 0)?.events ?? []),
    // the check sets the export ran, and their results as the app's Checks pane reads them
    check_sets_list: () => [...new Map(manifest.checks.map((c) => [c.set.name, { ...c.set, runs_on_any_file: true }])).values()],
    checks_run: (a) => manifest.checks.filter((c) => c.file === Number(a.file) && (a.sets as string[]).includes(c.set.name)).map((c) => ({ set: c.set.name, result: c.result, events: c.events, t_first: c.t_first })),
  };

  callbacks = installInternals(commands, location.search.includes('debug')).callbacks;
  (window as unknown as Record<string, unknown>).__tlmViewer = true;
}
