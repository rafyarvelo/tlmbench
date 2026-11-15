// Browser-only mode: TelemetryBench in a browser tab with no app behind it. The core is compiled to WASM (crates/web), files
// are opened from the page (the picker, or a drop) and read in memory by the decoder's own CSV reader, and what was read is kept
// in the browser's private file storage (OPFS) so opening the same file again skips the parse. Nothing is written beside the
// data, and nothing leaves the machine. (Design: Export to HTML › browser-only mode.)
//
// What it reads and does is smaller than the app's: CSV, Parquet and .tlm files, charts, tables, values and Measure. Binary formats, packet files,
// descriptors, plugins, folders, live sources, derived channels and checks are the app's (see docs/follow-ups.md).

import { isParquet, parquetDump } from './parquet-wasm';
import { type Commands, installInternals, loadCore, stubs } from './wasmcore';

export const LOCAL_PREFIX = 'local://';
const CACHE_DIR = 'tlmbench-cache';

type Handler = (data: unknown) => void;

/** The folder in the browser's private storage, or null where there is none (a private window, an old browser). */
async function cacheDir(): Promise<FileSystemDirectoryHandle | null> {
  try {
    return await (await navigator.storage.getDirectory()).getDirectoryHandle(CACHE_DIR, { create: true });
  } catch {
    return null;
  }
}

/** A name that finds the same file's cache entry again: its name and size, and a hash of its first and last megabyte (the
 *  modified time isn't used: a copy of the file, or a download again, is the same data). */
async function keyOf(f: File): Promise<string> {
  const MB = 1 << 20;
  const head = new Uint8Array(await f.slice(0, MB).arrayBuffer());
  const tail = new Uint8Array(await f.slice(Math.max(0, f.size - MB)).arrayBuffer());
  const h = new Uint8Array(await crypto.subtle.digest('SHA-256', new Uint8Array([...head.slice(0, 1 << 16), ...tail.slice(-(1 << 16))]).buffer));
  const hex = [...h.slice(0, 8)].map((b) => b.toString(16).padStart(2, '0')).join('');
  return `${f.name.replace(/[^\w.-]+/g, '_')}-${f.size}-${hex}`;
}

async function readCache(key: string): Promise<Uint8Array | null> {
  try {
    const dir = await cacheDir();
    if (!dir) return null;
    return new Uint8Array(await (await (await dir.getFileHandle(`${key}.bin`)).getFile()).arrayBuffer());
  } catch {
    return null;
  }
}

async function writeCache(key: string, bytes: Uint8Array<ArrayBuffer>): Promise<void> {
  try {
    const dir = await cacheDir();
    if (!dir) return;
    // under a temporary name, then in place: a tab closed mid-write leaves no half entry
    const tmp = await dir.getFileHandle(`${key}.part`, { create: true });
    const w = await tmp.createWritable();
    await w.write(bytes);
    await w.close();
    const final = await dir.getFileHandle(`${key}.bin`, { create: true });
    const out = await final.createWritable();
    await out.write(bytes);
    await out.close();
    await dir.removeEntry(`${key}.part`);
  } catch {
    /* no storage: the file is read again next time */
  }
}

/** Parses a file in a Worker: what the core keeps of it (its dump), or null where there is no Worker. A parse error throws. */
async function parseInWorker(file: File, path: string, wasm: string, parquetWasm: string): Promise<ArrayBuffer | null> {
  if (typeof Worker === 'undefined') return null;
  let w: Worker;
  try {
    w = new Worker(new URL('./browser-worker.ts', import.meta.url), { type: 'module' });
  } catch {
    return null;
  }
  return new Promise((done, fail) => {
    w.onmessage = (e: MessageEvent<{ dump?: ArrayBuffer; error?: string }>) => {
      w.terminate();
      if (e.data.dump) done(e.data.dump);
      else fail(new Error(e.data.error ?? 'The file could not be read.'));
    };
    w.onerror = () => {
      w.terminate();
      done(null);
    };
    w.postMessage({ wasm, parquetWasm, file, path });
  });
}

/** The file picker: the names it chose as `local://name` paths (the files are kept for `open`). */
function pick(files: Map<string, File>, multiple: boolean): Promise<string[] | null> {
  return new Promise((done) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = multiple;
    input.accept = '.csv,.tsv,.txt,.tlm,.parquet,text/csv';
    input.onchange = () => done(register(files, [...(input.files ?? [])]));
    input.oncancel = () => done(null);
    input.click();
  });
}

function register(files: Map<string, File>, list: File[]): string[] {
  return list.map((f) => {
    const path = `${LOCAL_PREFIX}${f.name}`;
    files.set(path, f);
    return path;
  });
}

const MANIFEST = 'tlmbench.local';

/** The files this browser has read, by their `local://` path: the name of the entry that holds each one's columns. A reload
 *  reopens them from there, with no picker. */
function manifest(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(MANIFEST) ?? '{}') as Record<string, string>;
  } catch {
    return {};
  }
}

function remember(path: string, key: string): void {
  try {
    localStorage.setItem(MANIFEST, JSON.stringify({ ...manifest(), [path]: key }));
  } catch {
    /* nothing remembered */
  }
}

/** A saved session, window or recent list that names a file this browser no longer holds can't reopen it: that entry is
 *  dropped, so the app doesn't try. Files it still holds stay, and a reload brings them back. */
async function forgetLocalFiles(): Promise<void> {
  try {
    const raw = localStorage.getItem('tlmbench.settings');
    if (!raw) return;
    const s = JSON.parse(raw) as Record<string, unknown>;
    const dir = await cacheDir();
    const held = new Set<string>();
    for (const [path, key] of Object.entries(manifest())) {
      try {
        await dir?.getFileHandle(`${key}.bin`);
        held.add(path);
      } catch {
        /* gone */
      }
    }
    const lost = (v: unknown) => [...JSON.stringify(v ?? '').matchAll(/local:\/\/[^"\\]*/g)].some((m) => !held.has(m[0]));
    for (const k of ['sessions', 'windows'] as const) if (lost(s[k])) delete s[k];
    if (Array.isArray(s.recent)) s.recent = s.recent.filter((p) => !String(p).startsWith(LOCAL_PREFIX) || held.has(String(p)));
    localStorage.setItem('tlmbench.settings', JSON.stringify(s));
  } catch {
    /* nothing saved yet */
  }
}

export async function installBrowserOnly(): Promise<void> {
  (globalThis as Record<string, unknown>).__tlmBrowserOnly = true;
  await forgetLocalFiles();
  const base = new URL('.', document.baseURI).href;
  const core = await loadCore(new URL('tlmbench_web.wasm', base).href);
  const files = new Map<string, File>();
  const ids = new Map<string, number>();
  let nextId = 0;
  let callbacks = new Map<number, Handler>();

  /** Opens a file from the page: from the private storage when it was read before, else parsed (and then kept). */
  async function openLocal(path: string, progress: Handler | null) {
    const f = files.get(path);
    if (!f) {
      // opened in an earlier visit: its columns are still in this browser's storage
      const saved = manifest()[path];
      const kept = saved ? await readCache(saved) : null;
      if (kept) {
        const k = nextId++;
        ids.set(path, k);
        const r = core('load', { file: k, path }, kept);
        (window as unknown as { __tlmCache?: string }).__tlmCache = 'restored';
        return r;
      }
      throw new Error(`${path.slice(LOCAL_PREFIX.length)} isn't open in this tab: choose it again, or drop it on the page.`);
    }
    if (!/\.(csv|tsv|txt|tlm|parquet)$/i.test(f.name)) throw new Error('Browser-only mode reads CSV, Parquet and .tlm files; the app reads more (packets, descriptors).');
    const k = nextId++;
    ids.set(path, k);
    const key = await keyOf(f);
    remember(path, key);
    const note = (done: number, total: number) => progress?.({ message: { done, total, builtUntil: null, error: null }, index: 0 });
    const cached = await readCache(key);
    if (cached) {
      try {
        const r = core('load', { file: k, path }, cached);
        (window as unknown as { __tlmCache?: string }).__tlmCache = 'hit';
        return r;
      } catch {
        /* a damaged entry: read the file again */
      }
    }
    note(0, f.size);
    // the parse runs in a Worker, so the tab keeps drawing and answering while a big file is read; where there is no Worker it
    // runs here, on the page's thread
    const dump = await parseInWorker(f, path, new URL('tlmbench_web.wasm', base).href, new URL('tlmbench_parquet.wasm', base).href);
    let r: unknown;
    if (dump) r = core('load', { file: k, path }, new Uint8Array(dump));
    else {
      await new Promise((res) => setTimeout(res, 0));
      const bytes = new Uint8Array(await f.arrayBuffer());
      r = isParquet(bytes) ? core('load', { file: k, path }, new Uint8Array(await parquetDump(new URL('tlmbench_parquet.wasm', base).href, bytes, path))) : core('import_file', { file: k, path }, bytes);
    }
    (window as unknown as { __tlmCache?: string; __tlmParse?: string }).__tlmCache = 'miss';
    (window as unknown as { __tlmParse?: string }).__tlmParse = dump ? 'worker' : 'page';
    // keep what was read, for next time, once the page is idle
    setTimeout(() => {
      try {
        void writeCache(key, new Uint8Array(dump ?? (core('dump', { file: k }) as ArrayBuffer)));
      } catch {
        /* nothing kept */
      }
    }, 50);
    return r;
  }

  const commands: Commands = {
    ...stubs,
    launch: () => ({ file: null, gate: null, gateMode: null, gateSeconds: null }),
    'plugin:dialog|open': (a) => {
      const o = (a.options ?? {}) as { directory?: boolean; multiple?: boolean };
      if (o.directory) return null; // no folders in a browser tab
      return pick(files, !!o.multiple).then((p) => (p == null ? null : o.multiple ? p : p[0]));
    },
    'plugin:dialog|save': () => null,
    'plugin:dialog|message': (a) => (confirm(String(a.message)) ? 'Yes' : 'No'),
    open: (a) => {
      const ch = a.progress as { id?: number } | string | undefined;
      const id = typeof ch === 'string' ? Number(ch.replace('__CHANNEL__:', '')) : ch?.id;
      return openLocal(String(a.path), id != null ? (callbacks.get(id) ?? null) : null);
    },
    close: (a) => core('close', { file: a.file }),
    view: (a) => core('view', a),
    values_at: (a) => core('values_at', a),
    stats: (a) => core('stats', a),
    rows: (a) => core('rows', a),
    events_list: () => [],
    cache_info: async () => {
      const dir = await cacheDir();
      let bytes = 0;
      let n = 0;
      if (dir) for await (const [, h] of (dir as unknown as { entries(): AsyncIterable<[string, FileSystemHandle]> }).entries()) if (h.kind === 'file') (n++, (bytes += (await (h as FileSystemFileHandle).getFile()).size));
      return { bytes, files: n, root: 'this browser' };
    },
    cache_clear: async () => {
      const root = await navigator.storage.getDirectory();
      await root.removeEntry(CACHE_DIR, { recursive: true }).catch(() => {});
      localStorage.removeItem(MANIFEST);
      return 0;
    },
  };

  const internals = installInternals(commands, location.search.includes('debug'));
  callbacks = internals.callbacks;

  // files dropped on the page open as they do in the app (the app listens for the window's drop event)
  const over = (e: DragEvent) => e.dataTransfer?.types.includes('Files');
  window.addEventListener('dragenter', (e) => over(e) && (e.preventDefault(), internals.emit('tauri://drag-enter', { paths: [], position: { x: e.clientX, y: e.clientY } })));
  window.addEventListener('dragover', (e) => over(e) && (e.preventDefault(), internals.emit('tauri://drag-over', { position: { x: e.clientX, y: e.clientY } })));
  window.addEventListener('dragleave', (e) => over(e) && internals.emit('tauri://drag-leave', {}));
  window.addEventListener('drop', (e) => {
    if (!over(e)) return;
    e.preventDefault();
    const paths = register(files, [...(e.dataTransfer?.files ?? [])]);
    internals.emit('tauri://drag-drop', { paths, position: { x: e.clientX, y: e.clientY } });
  });
  (window as unknown as { __tlmBrowserOnly: unknown }).__tlmBrowserOnly = { add: (list: File[]) => register(files, list) };
}
